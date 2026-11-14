//! The values that cross the boundary of a `Replica`: what it sends, what it reports.

use meldframe_protocol::Command;

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
}

/// What the replica did with a server event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventOutcome {
    /// My own pending op came back: it is confirmed.
    Confirmed,
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
