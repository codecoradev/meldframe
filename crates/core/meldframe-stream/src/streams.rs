//! The hubs of many files, keyed by a caller-chosen string: a hub exists while it has a stream, and a
//! server-wide cap bounds the streams across all of them.

use crate::{Config, Frame, Hub, StreamError, StreamId, Subscribed};
use std::collections::BTreeMap;

/// A subscription with its replay: the ops older than the window, read by the caller's `read`, to send
/// before the stream's queue.
#[derive(Debug, Clone)]
pub struct Joined<D> {
    pub id: StreamId,
    pub replay: Vec<D>,
}

/// Why a subscription with a replay failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinError<R> {
    Stream(StreamError),
    /// The caller's `read` of the older range failed.
    Read(R),
}

/// One [`Hub`] per key that has a subscriber.
#[derive(Debug)]
pub struct Streams<D, E> {
    config: Config,
    max_total: usize,
    next_id: u64,
    hubs: BTreeMap<String, Hub<D, E>>,
}

impl<D: Clone, E: Clone> Streams<D, E> {
    /// Hubs with `config` each, and at most `max_total` streams in all.
    pub fn new(config: Config, max_total: usize) -> Self {
        Streams {
            config,
            max_total,
            next_id: 0,
            hubs: BTreeMap::new(),
        }
    }

    /// Streams across all keys.
    pub fn stream_count(&self) -> usize {
        self.hubs.values().map(Hub::stream_count).sum()
    }

    /// Open a stream on `key`. `latest` is the key's newest sequence number, used when its hub is made.
    pub fn subscribe(
        &mut self,
        key: &str,
        latest: u64,
        from_seq: u64,
        now: u64,
    ) -> Result<Subscribed, StreamError> {
        if self.stream_count() >= self.max_total {
            return Err(StreamError::Full);
        }
        let config = self.config;
        // Ids come from here and are never reused, so a dropped hub's ids cannot reach a later client.
        let id = StreamId(self.next_id);
        self.next_id += 1;
        let hub = self
            .hubs
            .entry(key.to_string())
            .or_insert_with(|| Hub::new(config, latest));
        let sub = hub.subscribe_as(id, from_seq, now);
        self.drop_empty();
        sub
    }

    /// The client behind `id` was heard from (a ping, any message).
    pub fn seen(&mut self, key: &str, id: StreamId, now: u64) {
        if let Some(hub) = self.hubs.get_mut(key) {
            hub.seen(id, now);
        }
    }

    /// [`subscribe`](Self::subscribe), then `read(after, through)` for the range older than the window.
    /// A range longer than `max_replay` is read only in part: the stream then gets a resync marker at the
    /// cut, and the client resubscribes from it for the next part. A failed read leaves no stream behind.
    pub fn join<R>(
        &mut self,
        key: &str,
        latest: u64,
        from_seq: u64,
        now: u64,
        max_replay: u64,
        read: impl FnOnce(u64, u64) -> Result<Vec<D>, R>,
    ) -> Result<Joined<D>, JoinError<R>> {
        let sub = self
            .subscribe(key, latest, from_seq, now)
            .map_err(JoinError::Stream)?;
        let done = self.finish(key, sub, max_replay, read);
        if done.is_err() {
            self.unsubscribe(key, sub.id);
        }
        done
    }

    /// [`resubscribe`](Self::resubscribe), then `read` as in [`join`](Self::join). A failed read leaves
    /// the stream as it was.
    pub fn rejoin<R>(
        &mut self,
        key: &str,
        id: StreamId,
        from_seq: u64,
        now: u64,
        max_replay: u64,
        read: impl FnOnce(u64, u64) -> Result<Vec<D>, R>,
    ) -> Result<Joined<D>, JoinError<R>> {
        let old = self.hubs.get(key).and_then(|h| h.stream_copy(id));
        let sub = self
            .resubscribe(key, id, from_seq, now)
            .map_err(JoinError::Stream)?;
        let done = self.finish(key, sub, max_replay, read);
        if let (true, Some(old), Some(hub)) = (done.is_err(), old, self.hubs.get_mut(key)) {
            hub.put_stream(id, old);
        }
        done
    }

    fn finish<R>(
        &mut self,
        key: &str,
        sub: Subscribed,
        max_replay: u64,
        read: impl FnOnce(u64, u64) -> Result<Vec<D>, R>,
    ) -> Result<Joined<D>, JoinError<R>> {
        let Some((after, through)) = sub.missing else {
            return Ok(Joined {
                id: sub.id,
                replay: Vec::new(),
            });
        };
        let end = through.min(after.saturating_add(max_replay));
        let replay = read(after, end).map_err(JoinError::Read)?;
        if let (true, Some(hub)) = (end < through, self.hubs.get_mut(key)) {
            hub.cut(sub.id, end);
        }
        Ok(Joined { id: sub.id, replay })
    }

    /// Restart a stream of `key` from `from_seq`, keeping its slot.
    pub fn resubscribe(
        &mut self,
        key: &str,
        id: StreamId,
        from_seq: u64,
        now: u64,
    ) -> Result<Subscribed, StreamError> {
        let hub = self.hubs.get_mut(key).ok_or(StreamError::Unknown)?;
        hub.resubscribe(id, from_seq, now)
    }

    pub fn poll(&mut self, key: &str, id: StreamId, now: u64) -> Option<Frame<D, E>> {
        self.hubs.get_mut(key)?.poll(id, now)
    }

    /// Close a stream; the hub goes with its last stream.
    pub fn unsubscribe(&mut self, key: &str, id: StreamId) {
        if let Some(hub) = self.hubs.get_mut(key) {
            hub.unsubscribe(id);
        }
        self.drop_empty();
    }

    /// Publish durable op `seq` of `key` to its streams; nothing to do when it has none.
    pub fn publish(&mut self, key: &str, seq: u64, data: D) -> Result<(), StreamError> {
        self.hubs
            .get_mut(key)
            .map_or(Ok(()), |h| h.publish(seq, data))
    }

    /// Publish an ephemeral frame to one stream of `key` or all of them; returns how many queued it.
    pub fn publish_ephemeral(&mut self, key: &str, to: Option<StreamId>, data: E) -> usize {
        self.hubs
            .get_mut(key)
            .map_or(0, |h| h.publish_ephemeral(to, data))
    }

    /// Run every hub's heartbeat schedule; returns the `(key, stream)` pairs dropped as silent.
    pub fn tick(&mut self, now: u64) -> Vec<(String, StreamId)> {
        let dead = self
            .hubs
            .iter_mut()
            .flat_map(|(k, h)| {
                h.tick(now)
                    .into_iter()
                    .map(|id| (k.clone(), id))
                    .collect::<Vec<_>>()
            })
            .collect();
        self.drop_empty();
        dead
    }

    fn drop_empty(&mut self) {
        self.hubs.retain(|_, h| h.stream_count() > 0);
    }
}
