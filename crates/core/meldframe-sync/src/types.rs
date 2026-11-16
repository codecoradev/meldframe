//! The values that cross the boundary of a `Replica`: what it sends, what it reports.

use meldframe_check::LoadError;
use meldframe_core::ApplyError;
use meldframe_protocol::Command;
use std::fmt;

/// Why an op was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    /// A command the person gave.
    Edit,
    /// A command that undo worked out.
    Undo,
    /// A command that redo worked out.
    Redo,
}

/// An op the transport must send to the server. Sending it again with the same `key` is safe.
#[derive(Debug, Clone)]
pub struct Outgoing {
    /// Idempotency key, unique per replica.
    pub key: String,
    pub command: Command,
    pub kind: OpKind,
    /// The highest server sequence number the replica knew when it made this op.
    pub seen_seq: u64,
    /// Keys of my own ops that were still pending when this op was made and that it was built on
    /// (only for undo and redo). The server accepts the op only if every one of them was applied.
    pub deps: Vec<String>,
}

/// What the replica did with a server event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventOutcome {
    /// My own pending op came back: it is confirmed.
    Confirmed,
    /// Another actor's op: the confirmed state moved on and my pending ops were applied again on top of
    /// it. `dropped` is how many of them were no longer valid and were removed.
    Rebased { dropped: usize },
    /// The sequence number was applied before: ignored.
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplicaError {
    /// A sequence number is missing. Subscribe again from `seq()`, or load a snapshot.
    Gap { expected: u64, got: u64 },
    /// The event cannot be applied to the confirmed state. This should not happen; load the file again.
    Diverged(String),
}

/// Why a new local op was refused: nothing was applied, queued or recorded.
#[derive(Debug, Clone, PartialEq)]
pub enum SubmitError {
    /// The command is not valid in the view.
    Invalid(ApplyError),
    /// `max` ops are waiting for the server: wait for events, or resync (`docs/error-codes.md`).
    OutboxFull { max: usize },
}

impl SubmitError {
    /// The stable machine code (`docs/error-codes.md`); an invalid command keeps its own.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(e) => e.code(),
            Self::OutboxFull { .. } => "E_OUTBOX_FULL",
        }
    }
}

impl From<ApplyError> for SubmitError {
    fn from(e: ApplyError) -> Self {
        Self::Invalid(e)
    }
}

impl fmt::Display for SubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(e) => e.fmt(f),
            Self::OutboxFull { max } => write!(f, "{max} ops are waiting for the server"),
        }
    }
}

impl std::error::Error for SubmitError {}

/// Why a resync was refused: the replica is left as it was.
#[derive(Debug, Clone, PartialEq)]
pub enum ResyncError {
    /// The snapshot is not a valid file (it keeps the code of its `LoadError`).
    Load(LoadError),
    /// The snapshot is older than the events the replica already applied: loading it would go back.
    Behind { have: u64, got: u64 },
}

impl ResyncError {
    /// The stable machine code (`docs/error-codes.md`).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Load(e) => e.code(),
            Self::Behind { .. } => "E_SNAPSHOT_BEHIND",
        }
    }
}

impl From<LoadError> for ResyncError {
    fn from(e: LoadError) -> Self {
        Self::Load(e)
    }
}

impl fmt::Display for ResyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(e) => e.fmt(f),
            Self::Behind { have, got } => {
                write!(f, "the snapshot is at {got}, the replica is at {have}")
            }
        }
    }
}

impl std::error::Error for ResyncError {}
