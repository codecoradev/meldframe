//! The store seam: where a room keeps its log and snapshots.
//!
//! The log is append-only. Each op is kept with its actor, its idempotency key and a sequence number
//! (1, 2, 3 ... per room, no gaps). `meldframe-host` ships an in-memory adapter; Postgres comes in a
//! later change (E1.2). The trait is synchronous and holds no connection type, so the room stays
//! portable.

mod memory;

pub use memory::MemoryStore;

use meldframe_protocol::{Command, Node, Variables};
use std::fmt;

/// An op to append.
#[derive(Debug, Clone)]
pub struct NewOp {
    pub actor: String,
    /// Idempotency key, unique per actor within a room.
    pub key: String,
    pub command: Command,
}

/// An op in the log.
#[derive(Debug, Clone)]
pub struct StoredOp {
    pub seq: u64,
    pub actor: String,
    pub key: String,
    pub command: Command,
}

/// The result of an append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Appended {
    pub seq: u64,
    /// False when the key was already in the log: `seq` is the first write's and nothing was written.
    pub written: bool,
}

/// The tree and the variables of a room as they were after op `seq`.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub seq: u64,
    pub nodes: Vec<Node>,
    /// The collections and variables at `seq`; empty for a file without any.
    pub variables: Variables,
}

/// Why a store call failed. These are failures of the store, not refusals of an op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// A snapshot names a sequence number the log has not reached.
    SnapshotAhead { seq: u64, log: u64 },
    /// A snapshot is older than the latest one saved.
    SnapshotStale { seq: u64, latest: u64 },
    /// The key is already in the log for this actor, but with a different command: a client bug or a key
    /// collision. Nothing was written; `seq` is the first write's.
    KeyReused { seq: u64 },
    /// The adapter could not reach its backend (a database adapter uses this).
    Unavailable(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::SnapshotAhead { seq, log } => {
                write!(f, "snapshot at {seq} is ahead of the log at {log}")
            }
            StoreError::SnapshotStale { seq, latest } => {
                write!(f, "snapshot at {seq} is older than the latest at {latest}")
            }
            StoreError::KeyReused { seq } => {
                write!(f, "the key was used at {seq} for a different command")
            }
            StoreError::Unavailable(why) => write!(f, "store unavailable: {why}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// The seam. Every method takes the room id, so one store serves every room.
///
/// Contract for every adapter: an append is atomic per room (check the key, take the next number, write: no
/// gap, no duplicate `(room, actor, key)`). The room actor owns its store and calls it one call at a time,
/// which is why the methods take `&mut self`; an adapter over a shared backend keeps its own connection and
/// enforces the same rule there (for a database: a unique `(room, actor, key)` and a per-room counter).
pub trait Store {
    /// Append an op. A key that is already in the log for this actor in this room with the same command
    /// returns the first write's sequence number with `written: false` and writes nothing; with a different
    /// command it is refused as `KeyReused`.
    fn append(&mut self, room: &str, op: NewOp) -> Result<Appended, StoreError>;

    /// The sequence number of an actor's op by key, if it is in the log.
    fn find(&self, room: &str, actor: &str, key: &str) -> Result<Option<u64>, StoreError>;

    /// The highest sequence number in the room's log, 0 when it is empty.
    fn latest_seq(&self, room: &str) -> Result<u64, StoreError>;

    /// At most `limit` ops with a sequence number above `seq`, in order: the first page of the tail. To read
    /// further, call again with the last sequence number returned; an empty page means the tail is done.
    /// `limit` is a page size, so an adapter never loads a whole long log in one call; a `limit` of 0
    /// returns nothing (a caller that loops on pages must pass at least 1).
    fn ops_after(&self, room: &str, seq: u64, limit: usize) -> Result<Vec<StoredOp>, StoreError>;

    /// Keep a snapshot. Refused when it is ahead of the log or not newer than the latest snapshot. The caller
    /// builds the snapshot from the log up to `seq`; the store does not re-check the nodes.
    fn save_snapshot(&mut self, room: &str, snapshot: Snapshot) -> Result<(), StoreError>;

    /// The newest snapshot, if any.
    fn latest_snapshot(&self, room: &str) -> Result<Option<Snapshot>, StoreError>;
}
