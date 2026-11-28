//! The live streams of the server for the WebSocket route of the adapter: the [`LiveSource`] over the shared server.

use super::{SharedServer, failure_of};
use crate::{StreamId, SubmitError, SubscribeError, Subscription};
use meldframe_host::Principal;
use meldframe_http::Failure;
use meldframe_http_live::{LiveSource, Opened, Out};
use meldframe_protocol::ServerMsg;
use meldframe_rooms::RegistryError;

/// The live streams of the server, for the WebSocket route of the adapter.
impl LiveSource for SharedServer {
    fn subscribe(
        &self,
        who: &Principal,
        file: &str,
        from_seq: u64,
        now: u64,
    ) -> Result<Opened, Failure> {
        let sub = self.lock().subscribe(who, file, from_seq, now);
        opened(sub)
    }

    fn resubscribe(
        &self,
        who: &Principal,
        file: &str,
        id: StreamId,
        from_seq: u64,
        now: u64,
    ) -> Result<Opened, Failure> {
        let sub = self.lock().resubscribe(who, file, id, from_seq, now);
        opened(sub)
    }

    fn poll(&self, who: &Principal, file: &str, id: StreamId, now: u64, max: usize) -> Vec<Out> {
        let mut server = self.lock();
        std::iter::from_fn(|| server.poll(who, file, id, now))
            .take(max)
            .collect()
    }

    fn seen(&self, who: &Principal, file: &str, id: StreamId, now: u64) {
        self.lock().seen(who, file, id, now);
    }

    fn unsubscribe(&self, who: &Principal, file: &str, id: StreamId) {
        self.lock().unsubscribe(who, file, id);
    }

    fn heartbeat_ms(&self) -> Option<u64> {
        Some(self.lock().live_heartbeat_ms())
    }

    fn publish_ephemeral(&self, who: &Principal, file: &str, to: StreamId, msg: ServerMsg) -> bool {
        self.lock().publish_ephemeral(who, file, Some(to), msg) > 0
    }

    fn tick(&self, now: u64) -> Vec<StreamId> {
        // The file keys name every workspace's files: only the ids leave this function.
        let dropped = self.lock().tick(now);
        dropped.into_iter().map(|(_, id)| id).collect()
    }
}

/// A failed subscribe is not a refusal of the client's call: the stream cap, the registry and the store are
/// the server's to answer for, so the transport says `unavailable` (retryable).
fn opened(sub: Result<Subscription, SubscribeError>) -> Result<Opened, Failure> {
    sub.map(|s| Opened {
        id: s.id,
        replay: s.replay,
    })
    .map_err(|e| match e {
        // A bad name is the caller's to fix: `invalid`, never retryable.
        SubscribeError::Rooms(e @ RegistryError::InvalidName { .. }) => {
            failure_of(&SubmitError::Rooms(e))
        }
        e => Failure::Unavailable(format!("{e:?}")),
    })
}
