//! The live side of the adapter: the [`LiveSource`] seam over the server's streams, and the `LiveHub` the
//! connections and the timer share. The hub knows which connection holds which stream, so a stream the
//! server drops as silent closes its socket, and it wakes a file's connections when an op was accepted.
//! Stream ids stay in here: a client never sees one (they are sequential, and workspace is the only
//! check the server makes), so no other principal can name a connection's stream.

use meldframe_host::{Principal, StoredOp};
use meldframe_http::{Clock, Failure};
use meldframe_protocol::ServerMsg;
use meldframe_stream::{Frame, StreamId};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;

/// A frame as the server's stream hands it out.
pub type Out = Frame<StoredOp, ServerMsg>;

/// A new or renewed subscription: the stream, and the ops older than the server's window to send first.
#[derive(Debug)]
pub struct Opened {
    pub id: StreamId,
    pub replay: Vec<StoredOp>,
}

/// What the adapter needs from the server's live streams (`Server::subscribe` and the calls after it).
/// Implementations block on the server's store, so the adapter calls them from `spawn_blocking`.
pub trait LiveSource: Send + Sync {
    /// Open a stream on `file` of the principal's workspace for a client whose last number is `from_seq`.
    fn subscribe(
        &self,
        who: &Principal,
        file: &str,
        from_seq: u64,
        now: u64,
    ) -> Result<Opened, Failure>;
    /// Restart `id` from `from_seq`, the number in a resync marker.
    fn resubscribe(
        &self,
        who: &Principal,
        file: &str,
        id: StreamId,
        from_seq: u64,
        now: u64,
    ) -> Result<Opened, Failure>;
    /// Up to `max` queued frames of the stream, oldest first.
    fn poll(&self, who: &Principal, file: &str, id: StreamId, now: u64, max: usize) -> Vec<Out>;
    /// The client behind the stream sent something.
    fn seen(&self, who: &Principal, file: &str, id: StreamId, now: u64);
    /// Close a stream.
    fn unsubscribe(&self, who: &Principal, file: &str, id: StreamId);
    /// How often the server queues a heartbeat for a stream, in milliseconds, when it is known. The adapter
    /// never polls less often than this.
    fn heartbeat_ms(&self) -> Option<u64> {
        None
    }
    /// Queue heartbeats and drop silent or stalled streams at `now`; returns the ids dropped. The
    /// server's `(file key, id)` pairs name every workspace's files: only the ids are used, never sent.
    fn tick(&self, now: u64) -> Vec<StreamId>;
}

/// One open connection's handles.
pub(crate) struct Conn {
    /// Set when the server dropped the stream: the connection closes.
    pub(crate) dropped: Arc<Notify>,
    /// Set when an op of the connection's file was accepted.
    pub(crate) data: Arc<Notify>,
    key: String,
    id: StreamId,
}

/// Most dropped streams without a connection that the hub remembers.
const MAX_ORPHANS: usize = 256;

#[derive(Default)]
struct Tables {
    by_stream: HashMap<StreamId, Arc<Notify>>,
    by_file: HashMap<String, Arc<Notify>>,
    /// Streams the timer dropped while no connection held them: a socket still on its way up (subscribed at
    /// the handshake, not yet registered) must learn that its stream is gone. Ids are never reused. A
    /// socket that needed more than `MAX_ORPHANS` later drops to register would miss it, and then lives
    /// until its client leaves; the server's tick is the only reaper, and it has nothing left to reap.
    orphans: VecDeque<StreamId>,
}

/// Shared by the live connections and the timer.
pub(crate) struct LiveHub {
    pub(crate) source: Arc<dyn LiveSource>,
    pub(crate) clock: Clock,
    pub(crate) poll_every: Duration,
    tables: Mutex<Tables>,
}

fn file_key(who: &Principal, file: &str) -> String {
    format!("{}\u{0}{file}", who.workspace)
}

impl LiveHub {
    pub(crate) fn new(source: Arc<dyn LiveSource>, clock: Clock, poll_every: Duration) -> Self {
        LiveHub {
            source,
            clock,
            poll_every,
            tables: Mutex::default(),
        }
    }

    pub(crate) fn now(&self) -> u64 {
        (self.clock)()
    }

    fn tables(&self) -> std::sync::MutexGuard<'_, Tables> {
        self.tables.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Take the stream's place among the connections; `None` when the timer dropped the stream already.
    pub(crate) fn register(&self, who: &Principal, file: &str, id: StreamId) -> Option<Conn> {
        let key = file_key(who, file);
        let mut t = self.tables();
        if t.orphans.contains(&id) {
            return None;
        }
        let dropped = Arc::new(Notify::new());
        t.by_stream.insert(id, dropped.clone());
        let data = t.by_file.entry(key.clone()).or_default().clone();
        Some(Conn {
            dropped,
            data,
            key,
            id,
        })
    }

    /// Forget a connection; the file's wake-up goes with its last connection.
    pub(crate) fn deregister(&self, conn: Conn) {
        let mut t = self.tables();
        t.by_stream.remove(&conn.id);
        let last = t
            .by_file
            .get(&conn.key)
            .is_some_and(|n| Arc::strong_count(n) <= 2); // the table's and `conn.data`
        if last {
            t.by_file.remove(&conn.key);
        }
    }

    /// An op of `file` was accepted: its connections look for frames now.
    pub(crate) fn wake(&self, who: &Principal, file: &str) {
        if let Some(n) = self.tables().by_file.get(&file_key(who, file)) {
            n.notify_waiters();
        }
    }

    /// One pass of the timer: `Server::tick(now)`, then close the connections of the streams it dropped. It
    /// wakes no other connection: a heartbeat the tick queued is sent at the connection's next look, within
    /// `live_poll`.
    pub(crate) async fn tick(&self) {
        let (source, now) = (self.source.clone(), self.now());
        let Ok(gone) = tokio::task::spawn_blocking(move || source.tick(now)).await else {
            return;
        };
        let mut t = self.tables();
        for id in gone {
            if let Some(n) = t.by_stream.get(&id) {
                n.notify_one();
            } else {
                if t.orphans.len() >= MAX_ORPHANS {
                    t.orphans.pop_front();
                }
                t.orphans.push_back(id);
            }
        }
    }
}

/// The timer loop of the live streams: every `every` it calls `Server::tick` with the adapter's clock,
/// which queues heartbeats and drops silent clients. Without it no heartbeat is sent and a vanished client
/// keeps its stream. Spawn [`Timer::run`] on the runtime that serves the router.
pub struct Timer {
    pub(crate) hub: Arc<LiveHub>,
    pub(crate) every: Duration,
}

impl std::fmt::Debug for Timer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timer")
            .field("every", &self.every)
            .finish_non_exhaustive()
    }
}

impl Timer {
    /// Run until the task is dropped.
    pub async fn run(self) {
        let mut interval = tokio::time::interval(self.every.max(Duration::from_millis(1)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            self.hub.tick().await;
        }
    }
}
