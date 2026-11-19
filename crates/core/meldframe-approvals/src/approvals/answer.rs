//! The owner's answer with the rules of a workspace: see [`Approvals::answer`].

use super::{Approval, Approvals, DecideError, Verdict};
use meldframe_host::Principal;
use std::fmt;

/// Why [`Approvals::answer`] did nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswerError {
    /// An actor cannot answer its own request: the owner is somebody else.
    OwnRequest { id: String },
    /// Unknown (to this workspace), already decided, or expired.
    Decide(DecideError),
}

impl fmt::Display for AnswerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnswerError::OwnRequest { id } => {
                write!(
                    f,
                    "approval {id} is a request of this actor: its own answer does not count"
                )
            }
            AnswerError::Decide(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for AnswerError {}

impl Approvals {
    /// `decide` for a principal: the approval must be of the principal's workspace (another workspace's is
    /// `Unknown`) and not its own request (`OwnRequest`). `who.actor` is recorded as the one who decided. This checks
    /// who may not answer; who may is the caller's call (sign-in, M2).
    pub fn answer(
        &mut self,
        who: &Principal,
        id: &str,
        approved: bool,
        at: u64,
    ) -> Result<&Approval, AnswerError> {
        match self.get(id) {
            Some(a) if a.workspace == who.workspace && a.actor == who.actor => {
                return Err(AnswerError::OwnRequest { id: id.into() });
            }
            Some(a) if a.workspace == who.workspace => {}
            _ => return Err(AnswerError::Decide(DecideError::Unknown { id: id.into() })),
        }
        let verdict = if approved {
            Verdict::Approve
        } else {
            Verdict::Deny
        };
        self.decide(id, verdict, &who.actor, at)
            .map_err(AnswerError::Decide)
    }
}
