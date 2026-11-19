//! The approval path of the server (E2.4, ADR-0010): a call that the gate says Ask for opens an approval, the owner
//! decides, and the same call sent again is admitted once. The state machine is `meldframe-approvals`; this is how the
//! server uses it: when a call may ask, who may answer, and what goes to the call log.

use crate::{CallEntry, Server, now};
use meldframe_approvals::{AnswerError, Approval, ApprovalGate, ApprovalState};
use meldframe_host::{Gate, Principal};

/// How long an approval lives, pending or approved (15 minutes: a working default, the owner's to change).
pub const APPROVAL_TTL_MS: u64 = 15 * 60 * 1000;

/// Change the gate under an approval gate without losing the flag that lets an approved op through.
pub(crate) fn tune(gate: &mut ApprovalGate, f: impl FnOnce(Gate) -> Gate) {
    let inner = std::mem::take(&mut **gate);
    **gate = f(inner);
}

impl Server {
    /// Set how long an approval lives (default [`APPROVAL_TTL_MS`]). Approvals made before are forgotten.
    pub fn with_approval_ttl(mut self, ms: u64) -> Self {
        self.approvals = meldframe_approvals::Approvals::new(ms);
        self
    }

    /// The approvals of `workspace` that wait for an answer, oldest first.
    pub fn pending_approvals(&self, workspace: &str) -> Vec<Approval> {
        (self.approvals.pending(workspace).into_iter())
            .cloned()
            .collect()
    }

    /// The owner's answer to an approval of the principal's workspace: `approved` lets the same call through once,
    /// else it stays refused (rules of `Approvals::answer`). No route or tool calls this yet: the server cannot tell an
    /// owner from an agent before sign-in (M2), and an MCP tool would let an agent answer itself. One call-log entry.
    pub fn decide_approval(
        &mut self,
        who: &Principal,
        id: &str,
        approved: bool,
    ) -> Result<ApprovalState, AnswerError> {
        let at = now();
        let a = self.approvals.answer(who, id, approved, at)?;
        let (state, file) = (a.state, a.file.clone());
        (self.calls).record(CallEntry::approval_decision(at, who, &file, approved));
        Ok(state)
    }
}
