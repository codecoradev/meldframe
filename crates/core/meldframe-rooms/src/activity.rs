//! The activity log seam: who did what, when, at which risk tier, for every accepted op.
//!
//! The log is for people and for live events (E1.4); it is not the source of truth. The store's op log is.
//! An entry is written after its op is in the store, so a failed write here is reported and never fails
//! the op (see [`accept_logged`](crate::accept_logged)). The log is best effort: after a failed write or a
//! crash between the store append and the log write an accepted op has no entry, and a retry of the same
//! key cannot repair that (it records nothing). A gap in `seq` shows the loss, and the store's `ops_after`
//! is the source to repair from. The trait is synchronous and holds no connection type; a database
//! adapter comes later.

use meldframe_module::RiskTier;
use meldframe_protocol::Command;
use std::collections::HashMap;
use std::fmt;

/// What a command did, with a stable name that reads back from a store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKind {
    AddNode,
    UpdateNode,
    RemoveNode,
    Reparent,
    Reorder,
    /// Any variable command (`add_variable`, `set_variable_value`, a collection or a mode).
    Variable,
}

impl ActivityKind {
    /// Every kind, in a fixed order.
    pub const ALL: [ActivityKind; 6] = [
        ActivityKind::AddNode,
        ActivityKind::UpdateNode,
        ActivityKind::RemoveNode,
        ActivityKind::Reparent,
        ActivityKind::Reorder,
        ActivityKind::Variable,
    ];

    /// The stable name (`add_node`, `update_node`, `remove_node`, `reparent`, `reorder`, `variable`).
    pub fn as_str(self) -> &'static str {
        match self {
            ActivityKind::AddNode => "add_node",
            ActivityKind::UpdateNode => "update_node",
            ActivityKind::RemoveNode => "remove_node",
            ActivityKind::Reparent => "reparent",
            ActivityKind::Reorder => "reorder",
            ActivityKind::Variable => "variable",
        }
    }

    /// The kind with this name; `None` for a name this version does not know (written by a newer one).
    pub fn parse(name: &str) -> Option<ActivityKind> {
        ActivityKind::ALL.into_iter().find(|k| k.as_str() == name)
    }
}

/// One accepted op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityEntry {
    /// The op's sequence number in its room.
    pub seq: u64,
    /// Who: the principal's actor.
    pub actor: String,
    /// What: the command kind.
    pub kind: ActivityKind,
    /// What: the node the command is about (for a reparent or a reorder, the node that moves).
    pub node_id: String,
    /// When: the clock value the caller gave to `accept_logged`. The crate reads no wall clock, so the unit
    /// (for example milliseconds since the epoch) is the caller's.
    pub at: u64,
    /// The risk tier the gate decided the op at.
    pub tier: RiskTier,
}

impl ActivityEntry {
    /// The kind and the node id of a command.
    pub fn describe(cmd: &Command) -> (ActivityKind, String) {
        match cmd {
            Command::UpdateNode { id, .. } => (ActivityKind::UpdateNode, id.clone()),
            Command::AddNode { node, .. } => (ActivityKind::AddNode, node.id.clone()),
            Command::RemoveNode { id } => (ActivityKind::RemoveNode, id.clone()),
            Command::Reparent { id, .. } => (ActivityKind::Reparent, id.clone()),
            Command::Reorder { id, .. } => (ActivityKind::Reorder, id.clone()),
            // The first id a variable command touches (without the `$var:` or `$col:` prefix).
            other => (
                ActivityKind::Variable,
                meldframe_variables::touched(other)
                    .first()
                    .and_then(|t| t.split_once(':').map(|(_, id)| id.to_string()))
                    .unwrap_or_default(),
            ),
        }
    }
}

/// Why an activity log call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityError {
    /// The adapter could not reach its backend (a database adapter uses this).
    Unavailable(String),
    /// An entry whose sequence number is not above the last one kept for the room.
    OutOfOrder { seq: u64, last: u64 },
}

impl fmt::Display for ActivityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActivityError::Unavailable(why) => write!(f, "activity log unavailable: {why}"),
            ActivityError::OutOfOrder { seq, last } => {
                write!(f, "entry {seq} is not above the last entry {last}")
            }
        }
    }
}

impl std::error::Error for ActivityError {}

/// The seam. Every method takes the room id, so one log serves every room. Entries are recorded in sequence
/// order, one call at a time.
pub trait ActivityLog {
    /// Keep an entry for the room.
    fn record(&mut self, room: &str, entry: ActivityEntry) -> Result<(), ActivityError>;

    /// At most `limit` entries with a sequence number above `seq`, in order (the same paging rule as
    /// `Store::ops_after`).
    fn entries_after(
        &self,
        room: &str,
        seq: u64,
        limit: usize,
    ) -> Result<Vec<ActivityEntry>, ActivityError>;
}

/// A log that keeps everything in memory. For tests and for a room that does not need its activity to
/// survive a restart.
#[derive(Debug, Default)]
pub struct MemoryActivityLog {
    rooms: HashMap<String, Vec<ActivityEntry>>,
}

impl MemoryActivityLog {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ActivityLog for MemoryActivityLog {
    fn record(&mut self, room: &str, entry: ActivityEntry) -> Result<(), ActivityError> {
        let log = self.rooms.entry(room.to_string()).or_default();
        if let Some(last) = log.last().map(|e| e.seq)
            && entry.seq <= last
        {
            return Err(ActivityError::OutOfOrder {
                seq: entry.seq,
                last,
            });
        }
        log.push(entry);
        Ok(())
    }

    fn entries_after(
        &self,
        room: &str,
        seq: u64,
        limit: usize,
    ) -> Result<Vec<ActivityEntry>, ActivityError> {
        Ok(self
            .rooms
            .get(room)
            .map(|l| {
                l.iter()
                    .filter(|e| e.seq > seq)
                    .take(limit)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }
}
