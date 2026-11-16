//! Live event stream logic (ADR-0009), pure and synchronous: no transport, no async runtime, no threads and
//! no clock of its own (every call that needs time takes `now` in milliseconds). Portable to `wasm32`.
//!
//! One [`Hub`] serves the clients of one file. Durable frames are the accepted ops: each has a sequence
//! number and a client that reconnects with its last number is sent exactly the missed ones, from the
//! hub's bounded in-memory window and, for older ones, from the store, which the caller reads for the
//! range in [`Subscribed::missing`]. Ephemeral frames carry no number and are never replayed. Every
//! stream has a bounded queue; a full queue never blocks the publisher: the durable frame is dropped and
//! the stream gets one [`Frame::Resync`] marker, after which it receives nothing until it
//! [`resubscribes`](Hub::resubscribe) from the number in the marker. Streams are capped, and
//! [`Hub::tick`] queues heartbeats and drops clients that have been silent too long.

mod streams;
mod touch;

pub use streams::{JoinError, Joined, Streams};
pub use touch::{MAX_TOUCH_ENTRIES, MAX_TOUCH_NODES, TouchHint, Touches};

use std::collections::{BTreeMap, VecDeque};

/// Limits of a hub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Durable frames kept in memory for replay; older ones come from the store.
    pub window: usize,
    /// Frames queued per stream before it is resynced.
    pub queue: usize,
    /// Concurrent streams.
    pub max_streams: usize,
    /// Queue a heartbeat when nothing was sent for this long.
    pub heartbeat_ms: u64,
    /// Drop a stream the client has not been heard on (`seen`) for this long.
    pub dead_after_ms: u64,
    /// Drop a stream that has been stalled (resync pending) for this long, even if it keeps pinging.
    pub stall_ms: u64,
}

/// One client's stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamId(u64);

/// What a stream delivers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame<D, E> {
    /// An accepted op, numbered and replayable.
    Durable { seq: u64, data: D },
    /// Best effort, no number, never replayed.
    Ephemeral(E),
    /// Frames after `after_seq` were dropped for this stream (slow client, or a client ahead of the log);
    /// `latest_seq` is the newest number. Resubscribe from `after_seq`.
    Resync { after_seq: u64, latest_seq: u64 },
    /// Nothing to say; the connection is alive.
    Heartbeat,
}

/// Why a hub call failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamError {
    /// The stream cap is reached.
    Full,
    /// `publish` skipped a number.
    Gap { expected: u64, got: u64 },
    /// No such stream.
    Unknown,
}

/// A new (or renewed) subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subscribed {
    pub id: StreamId,
    /// Ops `(after, through]` are older than the window: read them from the store and send them before
    /// the stream's queue.
    pub missing: Option<(u64, u64)>,
}

#[derive(Debug, Clone)]
struct Stream<D, E> {
    queue: VecDeque<Frame<D, E>>,
    /// Newest durable number queued (or the cursor the client started from).
    last_seq: u64,
    /// Overflowed or ahead: no more durable frames until a resubscribe.
    stalled: bool,
    marker_sent: bool,
    /// When a tick first saw it stalled.
    stalled_since: Option<u64>,
    last_sent: u64,
    last_seen: u64,
}

/// The streams of one file.
#[derive(Debug)]
pub struct Hub<D, E> {
    config: Config,
    latest: u64,
    window: VecDeque<(u64, D)>,
    streams: BTreeMap<StreamId, Stream<D, E>>,
    next_id: u64,
}

impl<D: Clone, E: Clone> Hub<D, E> {
    /// A hub for a file whose last op is `latest`; the window starts empty.
    pub fn new(config: Config, latest: u64) -> Self {
        Hub {
            config,
            latest,
            window: VecDeque::new(),
            streams: BTreeMap::new(),
            next_id: 0,
        }
    }

    /// The newest sequence number published.
    pub fn latest(&self) -> u64 {
        self.latest
    }

    pub fn stream_count(&self) -> usize {
        self.streams.len()
    }

    /// Open a stream for a client whose last number is `from_seq` (0 for a new client).
    pub fn subscribe(&mut self, from_seq: u64, now: u64) -> Result<Subscribed, StreamError> {
        let id = StreamId(self.next_id);
        self.next_id += 1;
        self.subscribe_as(id, from_seq, now)
    }

    /// Open a stream under an id the caller keeps unique (the cap is the caller's to check).
    pub(crate) fn subscribe_as(
        &mut self,
        id: StreamId,
        from_seq: u64,
        now: u64,
    ) -> Result<Subscribed, StreamError> {
        if self.streams.len() >= self.config.max_streams {
            return Err(StreamError::Full);
        }
        Ok(self.attach(id, from_seq, now))
    }

    pub(crate) fn stream_copy(&self, id: StreamId) -> Option<Stream<D, E>> {
        self.streams.get(&id).cloned()
    }

    pub(crate) fn put_stream(&mut self, id: StreamId, stream: Stream<D, E>) {
        self.streams.insert(id, stream);
    }

    /// The replay for `id` was cut at `after_seq`: nothing is queued, and the stream gets a resync marker.
    pub(crate) fn cut(&mut self, id: StreamId, after_seq: u64) {
        if let Some(s) = self.streams.get_mut(&id) {
            s.queue.clear();
            s.last_seq = after_seq;
            s.stalled = true;
            s.marker_sent = false;
        }
    }

    /// Restart stream `id` from `from_seq` (after a resync marker), keeping its slot.
    pub fn resubscribe(
        &mut self,
        id: StreamId,
        from_seq: u64,
        now: u64,
    ) -> Result<Subscribed, StreamError> {
        if !self.streams.contains_key(&id) {
            return Err(StreamError::Unknown);
        }
        Ok(self.attach(id, from_seq, now))
    }

    fn attach(&mut self, id: StreamId, from_seq: u64, now: u64) -> Subscribed {
        let mut stream = Stream {
            queue: VecDeque::new(),
            last_seq: from_seq.min(self.latest),
            stalled: false,
            marker_sent: false,
            stalled_since: None,
            last_sent: now,
            last_seen: now,
        };
        let start = self.window.front().map_or(self.latest + 1, |(s, _)| *s);
        let mut missing = None;
        if from_seq > self.latest {
            stream.stalled = true;
        } else if from_seq < self.latest {
            if from_seq + 1 < start {
                missing = Some((from_seq, start - 1));
            }
            for (seq, data) in self.window.iter().filter(|(s, _)| *s > from_seq) {
                push_durable(&mut stream, self.config.queue, *seq, data);
            }
        }
        self.streams.insert(id, stream);
        Subscribed { id, missing }
    }

    pub fn unsubscribe(&mut self, id: StreamId) {
        self.streams.remove(&id);
    }

    /// Publish the accepted op `seq`. A number at or below the latest is a repeat and is ignored; a skipped
    /// number is a bug in the caller. Never waits for a client.
    pub fn publish(&mut self, seq: u64, data: D) -> Result<(), StreamError> {
        if seq <= self.latest {
            return Ok(());
        }
        if seq != self.latest + 1 {
            let expected = self.latest + 1;
            // A hub that missed a number cannot replay across it: every stream resyncs, and the store
            // covers the range (the window starts again after `seq`).
            self.latest = seq;
            self.window.clear();
            for s in self.streams.values_mut() {
                s.stalled = true;
                s.marker_sent = false;
            }
            return Err(StreamError::Gap { expected, got: seq });
        }
        self.latest = seq;
        self.window.push_back((seq, data.clone()));
        while self.window.len() > self.config.window {
            self.window.pop_front();
        }
        for stream in self.streams.values_mut() {
            push_durable(stream, self.config.queue, seq, &data);
        }
        Ok(())
    }

    /// Send an ephemeral frame to every stream (`None`) or to one. A full queue drops it. Returns how
    /// many streams queued it.
    pub fn publish_ephemeral(&mut self, to: Option<StreamId>, data: E) -> usize {
        let mut queued = 0;
        for (id, stream) in &mut self.streams {
            let wanted = to.is_none_or(|t| t == *id);
            if wanted && !stream.stalled && stream.queue.len() < self.config.queue {
                stream.queue.push_back(Frame::Ephemeral(data.clone()));
                queued += 1;
            }
        }
        queued
    }

    /// The next frame for `id`, if any.
    pub fn poll(&mut self, id: StreamId, now: u64) -> Option<Frame<D, E>> {
        let latest = self.latest;
        let stream = self.streams.get_mut(&id)?;
        let frame = match stream.queue.pop_front() {
            Some(f) => f,
            None if stream.stalled && !stream.marker_sent => {
                stream.marker_sent = true;
                Frame::Resync {
                    after_seq: stream.last_seq,
                    latest_seq: latest,
                }
            }
            None => return None,
        };
        stream.last_sent = now;
        Some(frame)
    }

    /// The client was heard from (a ping, any message).
    pub fn seen(&mut self, id: StreamId, now: u64) {
        if let Some(stream) = self.streams.get_mut(&id) {
            stream.last_seen = now;
        }
    }

    /// Advance the heartbeat schedule: queue a heartbeat on every idle stream that has been quiet for the
    /// interval, and remove (and return) streams not heard from for the dead time.
    pub fn tick(&mut self, now: u64) -> Vec<StreamId> {
        let cfg = self.config;
        let dead: Vec<StreamId> = self
            .streams
            .iter()
            .filter(|(_, s)| {
                let silent = now.saturating_sub(s.last_seen) >= cfg.dead_after_ms;
                let stuck = s
                    .stalled_since
                    .is_some_and(|t| now.saturating_sub(t) >= cfg.stall_ms);
                silent || stuck
            })
            .map(|(id, _)| *id)
            .collect();
        for id in &dead {
            self.streams.remove(id);
        }
        for stream in self.streams.values_mut() {
            if stream.stalled && stream.stalled_since.is_none() {
                stream.stalled_since = Some(now);
            }
            let quiet = now.saturating_sub(stream.last_sent) >= cfg.heartbeat_ms;
            if quiet && stream.queue.is_empty() && !(stream.stalled && !stream.marker_sent) {
                stream.queue.push_back(Frame::Heartbeat);
            }
        }
        dead
    }
}

fn push_durable<D: Clone, E>(stream: &mut Stream<D, E>, cap: usize, seq: u64, data: &D) {
    if stream.stalled {
        return;
    }
    if stream.queue.len() >= cap {
        stream.stalled = true;
        return;
    }
    stream.queue.push_back(Frame::Durable {
        seq,
        data: data.clone(),
    });
    stream.last_seq = seq;
}
