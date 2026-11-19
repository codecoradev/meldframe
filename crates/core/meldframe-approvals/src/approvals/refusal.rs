//! What a call through the approval seam is refused with, and why a decision did nothing.

use super::ApprovalState;
use meldframe_host::Refusal;
use std::fmt;

/// Why a call through [`Approvals::submit`] was refused. Codes are stable (`docs/error-codes.md`).
#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalRefusal {
    /// The policy is Ask and the owner has not approved: wait for the decision, then send the op again.
    Required { id: String },
    /// The owner denied this op. Send a new op (a new key) to ask again.
    Denied { id: String },
    /// The approval ran out before it was used. Send a new op (a new key) to ask again.
    Expired { id: String },
    /// Too many approvals are open (see [`Approvals::MAX_OPEN_PER_ACTOR`] and [`Approvals::MAX_OPEN_PER_WORKSPACE`]):
    /// wait for the owner to answer one, or for one to run out. `limit` is the cap that was hit.
    TooMany { limit: u64 },
    /// The gate refused for another reason (Deny policy, quota, shape); it keeps its own code.
    Refused(Refusal),
}

impl ApprovalRefusal {
    pub fn code(&self) -> &'static str {
        match self {
            ApprovalRefusal::Required { .. } => "approval_required",
            ApprovalRefusal::Denied { .. } => "approval_denied",
            ApprovalRefusal::Expired { .. } => "approval_expired",
            ApprovalRefusal::TooMany { .. } => "approval_limit",
            ApprovalRefusal::Refused(r) => r.code(),
        }
    }
}

impl ApprovalRefusal {
    /// The id of the approval behind the refusal; `None` for a refusal of the gate.
    pub fn id(&self) -> Option<&str> {
        match self {
            ApprovalRefusal::Required { id }
            | ApprovalRefusal::Denied { id }
            | ApprovalRefusal::Expired { id } => Some(id),
            ApprovalRefusal::TooMany { .. } | ApprovalRefusal::Refused(_) => None,
        }
    }
}

impl fmt::Display for ApprovalRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApprovalRefusal::Required { id } => {
                write!(f, "approval required: approval {id} waits for the owner")
            }
            ApprovalRefusal::Denied { id } => write!(f, "approval denied: approval {id}"),
            ApprovalRefusal::Expired { id } => write!(f, "approval expired: approval {id}"),
            // No number: it would tell one workspace how loaded the others are.
            ApprovalRefusal::TooMany { .. } => {
                write!(
                    f,
                    "approval limit: too many approvals are open; wait for the owner"
                )
            }
            ApprovalRefusal::Refused(r) => write!(f, "{r}"),
        }
    }
}

impl std::error::Error for ApprovalRefusal {}

/// Why `decide` did nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecideError {
    Unknown {
        id: String,
    },
    /// A decision is final: the approval is not pending.
    NotPending {
        id: String,
        state: ApprovalState,
    },
    /// The approval ran out before the decision.
    Expired {
        id: String,
    },
}

impl fmt::Display for DecideError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecideError::Unknown { id } => write!(f, "approval {id} is unknown"),
            DecideError::NotPending { id, state } => {
                write!(f, "approval {id} is already {state:?}")
            }
            DecideError::Expired { id } => write!(f, "approval {id} expired"),
        }
    }
}

impl std::error::Error for DecideError {}
