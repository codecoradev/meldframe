//! Why the gate or the room refused an op. Codes are stable; `docs/error-codes.md` lists them.

use meldframe_check::ApplyError;
use meldframe_module::{Policy, RiskTier};
use std::fmt;

/// A typed refusal. A refused op writes nothing, so the caller can correct it and send it again.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// The policy for this tier does not let the principal do it (`Ask` counts as forbidden until
    /// approval exists).
    Forbidden { tier: RiskTier, policy: Policy },
    /// The workspace used up its quota.
    QuotaExceeded { limit: u64 },
    /// The workspace sent more ops in the current minute than its rate allows. `retry_after_ms` is the time to the
    /// start of the next minute window, when the count starts again.
    RateExceeded { limit: u64, retry_after_ms: u64 },
    /// The batch holds more ops than one call may carry.
    BatchTooLarge { limit: u64 },
    /// The workspace used up its storage (the bytes of the ops it has written).
    StorageExceeded { limit: u64 },
    /// A node the command touches changed after the caller's `if_seq`.
    Conflict { node_id: String },
    /// The op is malformed before any apply: empty batch, empty or repeated key, too many dependencies.
    Invalid { reason: String },
    /// A dependency was not applied for this actor.
    DependencyRejected { key: String },
    /// The command itself was refused by the document core; it keeps its own `E_` code.
    Apply(ApplyError),
}

impl Refusal {
    /// The stable machine code. The match has no wildcard, so a new variant cannot be added without one.
    pub fn code(&self) -> &'static str {
        match self {
            Refusal::Forbidden { .. } => "forbidden",
            Refusal::QuotaExceeded { .. } => "quota_exceeded",
            Refusal::RateExceeded { .. } => "quota_ops_per_minute",
            Refusal::BatchTooLarge { .. } => "batch_too_large",
            Refusal::StorageExceeded { .. } => "quota_storage",
            Refusal::Conflict { .. } => "conflict",
            Refusal::Invalid { .. } => "invalid",
            Refusal::DependencyRejected { .. } => "dependency_rejected",
            Refusal::Apply(e) => e.code(),
        }
    }

    /// Whether sending the same op again later can succeed without changing it.
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Refusal::QuotaExceeded { .. } | Refusal::RateExceeded { .. }
        )
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Forbidden { tier, policy } => write!(
                f,
                "forbidden: policy {} does not allow a {} call",
                match policy {
                    Policy::Auto => "auto",
                    Policy::Ask => "ask",
                    Policy::Deny => "deny",
                },
                tier.as_str()
            ),
            Refusal::QuotaExceeded { limit } => {
                write!(f, "quota exceeded: the workspace limit is {limit} ops")
            }
            Refusal::RateExceeded {
                limit,
                retry_after_ms,
            } => write!(
                f,
                "quota exceeded: the workspace limit is {limit} ops per minute; try again in {retry_after_ms} ms"
            ),
            Refusal::BatchTooLarge { limit } => {
                write!(f, "a call takes at most {limit} ops")
            }
            Refusal::StorageExceeded { limit } => {
                write!(
                    f,
                    "quota exceeded: the workspace storage limit is {limit} bytes"
                )
            }
            Refusal::Conflict { node_id } => {
                write!(f, "conflict: node {node_id} changed after ifSeq")
            }
            Refusal::Invalid { reason } => write!(f, "invalid: {reason}"),
            Refusal::DependencyRejected { key } => {
                write!(f, "dependency rejected: {key} was not applied")
            }
            Refusal::Apply(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Refusal {}

impl From<ApplyError> for Refusal {
    fn from(e: ApplyError) -> Self {
        Refusal::Apply(e)
    }
}
