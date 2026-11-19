//! The call log of the server (E2.4): one entry per call, kept by [`Server`]; see `meldframe-calls`.

use crate::{CallEntry, CallLog, Server, SubmitError, now};
use meldframe_host::{Principal, Refusal};
use meldframe_module::RiskTier;

impl Server {
    /// Keep one entry for a write call that ended in `failure` (`None`: it was written). `submit_tool` does this by
    /// itself; a caller that refuses a call before it reaches `submit_tool` (an atomic batch that failed its
    /// check) calls it for that call.
    pub fn record_call(
        &mut self,
        tool: &str,
        file: &str,
        who: &Principal,
        failure: Option<&SubmitError>,
    ) {
        // The approval path: a call that asks, or whose approval was refused; the tier asked about is Delete.
        if let Some(SubmitError::Approval(a)) = failure {
            let entry = CallEntry::with_code(
                now(),
                who,
                tool,
                file,
                Some(RiskTier::Delete),
                Some(a.code()),
            );
            self.calls.record(entry);
            return;
        }
        self.record_call_as(
            tool,
            file,
            who,
            failure.map_or(Ok(()), |e| Err(e.refusal())),
        );
    }

    /// The same for a caller that has only the refusal: `Ok(())` for a call that went through, `Err(Some(refusal))`
    /// for one a policy or the room refused, `Err(None)` for a failure that is not a refusal.
    pub fn record_call_as(
        &mut self,
        tool: &str,
        file: &str,
        who: &Principal,
        done: Result<(), Option<&Refusal>>,
    ) {
        let tier = self.registry.agent_tool(tool).map(|t| t.tier);
        self.calls
            .record(CallEntry::new(now(), who, tool, file, tier, done));
    }

    /// The entry for a call that does not go through the gate (`whoami`, a tool of another crate such as
    /// `capture`): `code` is `None` when it succeeded, else its stable error code. The tier is the tool's in the
    /// registry.
    pub fn record_call_code(
        &mut self,
        tool: &str,
        file: &str,
        who: &Principal,
        code: Option<&str>,
    ) {
        let tier = self.registry.agent_tool(tool).map(|t| t.tier);
        self.calls
            .record(CallEntry::with_code(now(), who, tool, file, tier, code));
    }

    /// At most `limit` of the newest calls, oldest first.
    pub fn calls(&self, limit: usize) -> Vec<CallEntry> {
        self.calls.recent(limit)
    }

    /// Replace the call log (an embedder that keeps it elsewhere); the default keeps the newest [`MAX_CALL_LOG`]
    /// in memory.
    pub fn with_call_log(mut self, log: Box<dyn CallLog>) -> Self {
        self.calls = log;
        self
    }
}
