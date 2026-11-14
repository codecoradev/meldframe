//! The state of one file in memory.

use crate::history::Entry;
use meldframe_core::{ApplyError, Limits, apply_planned, plan_command};
use meldframe_protocol::{Command, Node};

/// The in-memory tree of one file. Every change goes through [`FileState::apply`], which validates first
/// and changes after, so a refused command leaves the tree as it was.
#[derive(Debug, Clone, Default)]
pub struct FileState {
    pub(crate) nodes: Vec<Node>,
    limits: Limits,
    version: u64,
    pub(crate) undo: Vec<Entry>,
    pub(crate) redo: Vec<Entry>,
}

impl FileState {
    /// An empty file with the default limits.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty file with the given limits.
    pub fn with_limits(limits: Limits) -> Self {
        FileState {
            limits,
            ..Self::default()
        }
    }

    /// A file that starts from a loaded tree (a snapshot). The nodes must already carry their `pos`.
    pub fn from_nodes(nodes: Vec<Node>) -> Self {
        FileState {
            nodes,
            ..Self::default()
        }
    }

    /// All nodes. The list order carries no meaning: sibling order is `(pos, id)`.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The node with this id.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Goes up by one each time the tree changes.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Validate and apply a command and record it in the history (a new edit discards redo). A refused
    /// command changes nothing, not even the version.
    pub fn apply(&mut self, cmd: &Command) -> Result<(), ApplyError> {
        let inverse = self.run(cmd)?;
        self.record(cmd, inverse);
        Ok(())
    }

    /// Apply without touching the history; returns the inverse, worked out before the change.
    pub(crate) fn run(&mut self, cmd: &Command) -> Result<Vec<Command>, ApplyError> {
        let pos = plan_command(&self.nodes, cmd, &self.limits)?;
        let inverse = self.invert(cmd);
        apply_planned(&mut self.nodes, cmd, pos);
        self.version += 1;
        Ok(inverse)
    }
}
