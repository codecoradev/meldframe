//! The state of one file in memory.

use crate::history::Entry;
use crate::runs::Run;
use meldframe_check::{LoadError, check_file};
use meldframe_core::{
    ApplyError, Limits, apply_planned, normalize_positions, normalize_positions_with, plan_command,
};
use meldframe_protocol::{Command, Node};

/// The in-memory tree of one file. Every change goes through [`FileState::apply`], which validates first
/// and changes after, so a refused command leaves the tree as it was.
#[derive(Debug, Clone, Default)]
pub struct FileState {
    pub(crate) nodes: Vec<Node>,
    pub(crate) limits: Limits,
    pub(crate) version: u64,
    pub(crate) undo: Vec<Entry>,
    pub(crate) redo: Vec<Entry>,
    /// The step being built by `begin_group`, if any.
    pub(crate) group: Option<Entry>,
    /// The key of the edit that was recorded last, while it is still the top step: only that step takes
    /// a later edit with the same key. An undo, a redo or a group boundary closes it.
    pub(crate) merge_key: Option<String>,
    /// The id of the last rec made (ids start at 1).
    pub(crate) next_rec: u64,
    /// New recs are marked pending until confirmed (set by a replica that waits for the server).
    pub(crate) optimistic: bool,
    /// The id of the last step made (ids start at 1).
    pub(crate) next_entry: u64,
    /// Undo and redo steps whose ops have not been confirmed yet (optimistic mode only).
    pub(crate) runs: Vec<Run>,
    pub(crate) next_run: u64,
    pub(crate) last_run: Option<u64>,
    /// Goes up each time an edit discards a redo stack that was not empty.
    pub(crate) redo_epoch: u64,
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

    /// Load a stored file. The old-file migration runs first (siblings without a valid or unique `pos` get
    /// one by list order; a file already in order is not touched), then the whole file is checked with
    /// `check_file`. A bad file is refused with a typed error and nothing is repaired. The history starts
    /// empty.
    pub fn try_from_nodes(mut nodes: Vec<Node>, limits: Limits) -> Result<Self, LoadError> {
        normalize_positions_with(&mut nodes, &limits);
        check_file(&nodes, &limits)?;
        Ok(FileState {
            nodes,
            limits,
            ..Self::default()
        })
    }

    /// Like [`FileState::try_from_nodes`] with the default limits, but without the check: for a tree this
    /// process already holds as valid (a replica's snapshot from its own server). Never use it on bytes
    /// read from a store; that is what `try_from_nodes` is for.
    pub fn from_nodes_unchecked(mut nodes: Vec<Node>) -> Self {
        normalize_positions(&mut nodes);
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
        self.apply_keyed(cmd, None)
    }

    /// Like [`FileState::apply`]. When `key` equals the key of the edit recorded just before (no group,
    /// undo or redo in between), the command joins that step, so typing into one text is one step to undo.
    pub fn apply_keyed(&mut self, cmd: &Command, key: Option<&str>) -> Result<(), ApplyError> {
        let inverse = self.run(cmd)?;
        self.record(cmd, inverse, key);
        Ok(())
    }

    /// Replace the tree with the result of a rebase and keep the history as it is.
    pub fn reset_nodes(&mut self, nodes: Vec<Node>) {
        self.nodes = nodes;
        self.version += 1;
    }

    /// Apply my pending command again on a new tree; returns its inverse for the caller to give to the
    /// rec (`set_undo`). A command that no longer applies returns the error and changes nothing.
    pub fn replay_pending(&mut self, cmd: &Command) -> Result<Vec<Command>, ApplyError> {
        self.run(cmd)
    }

    /// Give the rec this undo, so it holds the value that is right in the server's order.
    pub fn set_undo(&mut self, id: u64, undo: Vec<Command>) {
        for r in self.recs_mut().filter(|r| r.id == id) {
            r.undo_ = undo.clone();
        }
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
