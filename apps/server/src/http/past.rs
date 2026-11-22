//! The reads behind `GET /v1/files/{file}/checkpoints/{n}/state` (ADR-0023): the newest snapshot of a file and a page of
//! its log, each one call under the server lock on one opening of the file, not logged here (the route writes the one
//! entry of the call) and allowed only when the registry has `checkpoint_state` with an `Auto` policy.

use super::SharedServer;
use meldframe_host::{Principal, Room, Store, StoreError, StoredOp};
use meldframe_http::Failure;
use meldframe_http_checkpoints::{Log, Past, STATE_TOOL};
use meldframe_module::Policy;
use meldframe_protocol::Node;

fn unavailable(e: StoreError) -> Failure {
    Failure::Unavailable(e.to_string())
}

impl SharedServer {
    /// Run `f` on one opening of the file, under the policy of `checkpoint_state`.
    fn past<T>(
        &self,
        who: &Principal,
        file: &str,
        f: impl FnOnce(&Room<crate::AnyStore>) -> T,
    ) -> Result<T, Failure> {
        if self.lock().registry().policy_for(STATE_TOOL) != Some(Policy::Auto) {
            let tier = meldframe_module::RiskTier::Read;
            let policy = Policy::Deny;
            return Err(Failure::Refused(meldframe_host::Refusal::Forbidden {
                tier,
                policy,
            }));
        }
        self.with_open_room(who, file, f)
    }
}

/// The log of one open room.
struct RoomLog<'a>(&'a Room<crate::AnyStore>);

impl Log for RoomLog<'_> {
    fn snapshot(&self) -> Result<Option<(u64, Vec<Node>)>, Failure> {
        let found = self.0.store().latest_snapshot(self.0.id());
        Ok(found.map_err(unavailable)?.map(|s| (s.seq, s.nodes)))
    }

    fn ops_after(&self, after: u64, limit: usize) -> Result<Vec<StoredOp>, Failure> {
        let page = self.0.store().ops_after(self.0.id(), after, limit);
        page.map_err(unavailable)
    }
}

impl Past for SharedServer {
    fn with_log(
        &self,
        who: &Principal,
        file: &str,
        f: &mut dyn FnMut(&dyn Log),
    ) -> Result<(), Failure> {
        self.past(who, file, |r| f(&RoomLog(r)))
    }
}
