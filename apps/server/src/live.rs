//! Live streams of the server: a `meldframe_stream::Streams` (one hub per file with a subscriber) fed by
//! `Server::submit_tool` with the ops the room accepted (ADR-0009). There is no transport here: a
//! transport calls `subscribe`, sends the replay and then drains `poll`.
//!
//! Ownership: a stream belongs to the actor that opened it. `poll`, `seen`, `resubscribe`, `unsubscribe`
//! do nothing for another actor, or for another workspace (the
//! file key holds the workspace). `publish_ephemeral` is the server's own push (the capture tool asks the
//! workspace's editors) and is scoped by workspace only. Ids are sequential, so a transport should still keep a `StreamId` inside
//! its connection task and never show it to a client. A room opened for a stream is not closed on
//! unsubscribe or reaping; only idle eviction (or `close`) frees it, so streams on many files keep their
//! rooms open until then.

use crate::{Server, limits, room_key};
use meldframe_host::{Principal, Store, StoreError, StoredOp};
use meldframe_protocol::ServerMsg;
use meldframe_rooms::RegistryError;
use meldframe_stream::{Config, JoinError, Joined, Streams};

pub use meldframe_stream::{Frame, StreamError, StreamId};

/// Per-file stream limits.
pub const LIVE_CONFIG: Config = Config {
    window: 256,
    // At least the window, so a client that attaches from the oldest kept number does not overflow.
    queue: 256,
    max_streams: 64,
    heartbeat_ms: 15_000,
    dead_after_ms: 45_000,
    stall_ms: 30_000,
};
/// The most ops one subscribe reads from the store; a longer gap is cut with a resync marker and the
/// client resubscribes from it.
pub const MAX_REPLAY: u64 = 1024;
/// Concurrent streams across all files.
pub const MAX_LIVE_STREAMS: usize = 1024;

pub(crate) type Live = Streams<StoredOp, ServerMsg>;

/// The stream and the ops it missed that are no longer in memory (read from the store; send them first).
pub type Subscription = Joined<StoredOp>;

/// Why a subscription failed: the file would not open, the stream cap is reached (or the stream is
/// unknown), or the store could not be read for the replay.
#[derive(Debug)]
pub enum SubscribeError {
    Rooms(RegistryError),
    Stream(StreamError),
    Store(StoreError),
}

impl From<JoinError<StoreError>> for SubscribeError {
    fn from(e: JoinError<StoreError>) -> Self {
        match e {
            JoinError::Stream(e) => SubscribeError::Stream(e),
            JoinError::Read(e) => SubscribeError::Store(e),
        }
    }
}

/// At most one touch hint per actor and file in this many milliseconds.
pub const TOUCH_WINDOW_MS: u64 = 100;

impl Server {
    /// Use these per-file stream limits and this server-wide cap (see [`LIVE_CONFIG`]). Call it before
    /// [`Server::with_touch_hints`], which it resets.
    pub fn with_live(mut self, config: Config, max_streams: usize) -> Self {
        self.live = Streams::new(config, max_streams);
        self
    }

    /// Queue an ephemeral `touch` frame (ids only) on the streams of a file when ops of it are written, at
    /// most one per actor and file per `window_ms` (`Streams::touch`). Off by default.
    pub fn with_touch_hints(mut self, window_ms: u64) -> Self {
        self.live.set_touch(window_ms, |h| ServerMsg::Touch {
            actor: h.actor,
            node_ids: h.node_ids,
            seq: h.seq,
        });
        self
    }

    /// Streams whose owner the server knows. It equals [`Server::live_stream_count`] unless a stream was
    /// dropped by a path that does not tell the server (a leak check for operators and tests).
    pub fn live_owner_count(&self) -> usize {
        self.owners.len()
    }

    /// How often a stream is sent a heartbeat, in milliseconds.
    pub fn live_heartbeat_ms(&self) -> u64 {
        self.live.config().heartbeat_ms
    }

    /// Streams open now, across all files.
    pub fn live_stream_count(&self) -> usize {
        self.live.stream_count()
    }

    /// Open a stream on `file` of the principal's workspace for a client whose last sequence number is
    /// `from_seq` (0 when new). `now` is the caller's clock in milliseconds, the same for every live call.
    /// A failed call leaves no stream behind and closes a room it opened.
    pub fn subscribe(
        &mut self,
        who: &Principal,
        file: &str,
        from_seq: u64,
        now: u64,
    ) -> Result<Subscription, SubscribeError> {
        self.join_live(who, file, None, from_seq, now)
    }

    /// Restart stream `id` from `from_seq`, the number in a resync marker; a failed call leaves the stream
    /// as it was. A replay longer than [`MAX_REPLAY`] is cut with another marker.
    pub fn resubscribe(
        &mut self,
        who: &Principal,
        file: &str,
        id: StreamId,
        from_seq: u64,
        now: u64,
    ) -> Result<Subscription, SubscribeError> {
        self.join_live(who, file, Some(id), from_seq, now)
    }

    fn join_live(
        &mut self,
        who: &Principal,
        file: &str,
        again: Option<StreamId>,
        from_seq: u64,
        at: u64,
    ) -> Result<Subscription, SubscribeError> {
        limits::valid_names(&who.workspace, file).map_err(SubscribeError::Rooms)?;
        if again.is_some_and(|id| {
            !self
                .owners
                .owns(id, &room_key(&who.workspace, file), &who.actor)
        }) {
            return Err(SubscribeError::Stream(StreamError::Unknown));
        }
        let key = room_key(&who.workspace, file);
        let opened = self
            .open_room(&who.workspace, &key, crate::now())
            .map_err(SubscribeError::Rooms)?;
        // `close_all` reaches only the keys recorded here, so a room a stream opened must be one of them.
        self.open_keys.insert(key.clone());
        let Server { rooms, live, .. } = self;
        let read = |a, b| read_range(rooms, &key, a, b);
        let done = match again {
            None => live.join(&key, opened.seq, from_seq, at, MAX_REPLAY, read),
            Some(id) => live.rejoin(&key, id, from_seq, at, MAX_REPLAY, read),
        };
        if done.is_err() && !opened.already_open {
            self.rooms.close(&key);
            self.open_keys.remove(&key);
        }
        let joined = done?;
        self.owners.set(joined.id, &key, &who.actor);
        Ok(joined)
    }

    /// The next frame for a stream of the principal's workspace, if any. Another workspace's stream is
    /// not found. Polling is not proof of life and does not touch the stream's last-seen time: the
    /// transport calls [`Server::seen`] when the client sends something, or [`Server::tick`] drops the
    /// stream as silent.
    pub fn poll(
        &mut self,
        who: &Principal,
        file: &str,
        id: StreamId,
        now: u64,
    ) -> Option<Frame<StoredOp, ServerMsg>> {
        if !self
            .owners
            .owns(id, &room_key(&who.workspace, file), &who.actor)
        {
            return None;
        }
        self.live.flush_touches(crate::now());
        self.live.poll(&room_key(&who.workspace, file), id, now)
    }

    /// The client behind a stream was heard from (a ping, any message).
    pub fn seen(&mut self, who: &Principal, file: &str, id: StreamId, now: u64) {
        if self
            .owners
            .owns(id, &room_key(&who.workspace, file), &who.actor)
        {
            self.live.seen(&room_key(&who.workspace, file), id, now);
        }
    }

    /// Run the heartbeat schedule of every stream at `now`: queue heartbeats, and drop streams not seen
    /// for `dead_after_ms` or stalled for `stall_ms`, freeing their slots, and close rooms idle past `idle_after` (the same wall clock as writes). Returns the `(file key, stream)`
    /// pairs dropped. The transport calls this on a timer. The pairs are for the transport to find the
    /// connections to close, by stream id; the file key names another workspace's file too, so a
    /// transport must never forward them to a client.
    pub fn tick(&mut self, now: u64) -> Vec<(String, StreamId)> {
        self.close_idle(crate::now());
        self.live.flush_touches(crate::now());
        let dropped = self.live.tick(now);
        for (_, id) in &dropped {
            self.owners.forget(*id);
        }
        dropped
    }

    /// Close a stream of the principal's workspace; the file's hub goes with its last stream.
    pub fn unsubscribe(&mut self, who: &Principal, file: &str, id: StreamId) {
        if self
            .owners
            .owns(id, &room_key(&who.workspace, file), &who.actor)
        {
            self.live.unsubscribe(&room_key(&who.workspace, file), id);
            self.owners.forget(id);
        }
    }

    /// Send an ephemeral frame (a capture request, a touch hint) to one stream of `file` in the
    /// principal's workspace, or to all of them.
    pub fn publish_ephemeral(
        &mut self,
        who: &Principal,
        file: &str,
        to: Option<StreamId>,
        msg: ServerMsg,
    ) -> usize {
        self.live
            .publish_ephemeral(&room_key(&who.workspace, file), to, msg)
    }

    /// Send an ephemeral frame to every stream of `file` in the principal's workspace that keeps the touch
    /// headroom free (`Streams::publish_ephemeral_with_headroom`): the batch frame of ADR-0021 goes this way, so
    /// it never takes the last slots of a queue (it does not cap its share of the rest).
    pub fn publish_with_headroom(&mut self, who: &Principal, file: &str, msg: ServerMsg) -> usize {
        let key = room_key(&who.workspace, file);
        self.live.publish_ephemeral_with_headroom(&key, None, msg)
    }
}

/// The ops `(after, through]` of an open room from the store, a page at a time.
fn read_range(
    rooms: &crate::Rooms,
    key: &str,
    after: u64,
    through: u64,
) -> Result<Vec<StoredOp>, StoreError> {
    rooms
        .with_room(key, |room| {
            let (mut out, mut cursor) = (Vec::new(), after);
            while cursor < through {
                let page = room.store().ops_after(room.id(), cursor, 256)?;
                let Some(last) = page.last().map(|op| op.seq) else {
                    break;
                };
                out.extend(page.into_iter().filter(|op| op.seq <= through));
                cursor = last;
            }
            Ok(out)
        })
        .expect("the room was just opened")
}
