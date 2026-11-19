//! The state of one file in memory.

use meldframe_check::{LoadError, check_file_with};
use meldframe_core::{
    ApplyError, Limits, apply_planned_doc, normalize_positions, normalize_positions_with, plan_doc,
};
use meldframe_protocol::{Command, Doc, Node, Variables};
use meldframe_undo::History;

/// The in-memory tree of one file. Every change goes through [`FileState::apply`], which validates first
/// and changes after, so a refused command leaves the tree as it was.
#[derive(Debug, Clone, Default)]
pub struct FileState {
    pub(crate) doc: Doc,
    pub(crate) limits: Limits,
    pub(crate) version: u64,
    /// Undo and redo steps, the open group and the runs that wait for the server.
    pub(crate) hist: History,
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
    pub fn try_from_nodes(nodes: Vec<Node>, limits: Limits) -> Result<Self, LoadError> {
        Self::try_from_doc(Doc::of_nodes(nodes), limits)
    }

    /// Like [`FileState::try_from_nodes`] for a file with variables: the variables are checked first, then the
    /// nodes and their bindings (`check_file_with`).
    pub fn try_from_doc(mut doc: Doc, limits: Limits) -> Result<Self, LoadError> {
        normalize_positions_with(&mut doc.nodes, &limits);
        check_file_with(&doc.nodes, &doc.vars, &limits)?;
        Ok(FileState {
            doc,
            limits,
            ..Self::default()
        })
    }

    /// Like [`FileState::try_from_nodes`] with the default limits, but without the check: for a tree this
    /// process already holds as valid (a replica's snapshot from its own server). Never use it on bytes
    /// read from a store; that is what `try_from_nodes` is for.
    pub fn from_nodes_unchecked(nodes: Vec<Node>) -> Self {
        Self::from_doc_unchecked(Doc::of_nodes(nodes))
    }

    /// Like [`FileState::from_nodes_unchecked`] for a document with variables.
    pub fn from_doc_unchecked(mut doc: Doc) -> Self {
        normalize_positions(&mut doc.nodes);
        FileState {
            doc,
            ..Self::default()
        }
    }

    /// All nodes. The list order carries no meaning: sibling order is `(pos, id)`.
    pub fn nodes(&self) -> &[Node] {
        &self.doc.nodes
    }

    /// The collections and variables of the file.
    pub fn vars(&self) -> &Variables {
        &self.doc.vars
    }

    /// The nodes and the variables.
    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    /// The node with this id.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.doc.nodes.iter().find(|n| n.id == id)
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
        self.hist.record(cmd, inverse, key);
        Ok(())
    }

    /// Replace the tree with the result of a rebase and keep the history as it is.
    pub fn reset_nodes(&mut self, nodes: Vec<Node>) {
        self.reset_doc(Doc::of_nodes(nodes));
    }

    /// Replace the nodes and the variables with the result of a rebase and keep the history as it is.
    pub fn reset_doc(&mut self, doc: Doc) {
        self.doc = doc;
        self.version += 1;
    }

    /// Apply my pending command again on a new tree; returns its inverse for the caller to give to the
    /// rec (`set_undo`). A command that no longer applies returns the error and changes nothing.
    pub fn replay_pending(&mut self, cmd: &Command) -> Result<Vec<Command>, ApplyError> {
        self.run(cmd)
    }

    /// Give the rec this undo, so it holds the value that is right in the server's order.
    pub fn set_undo(&mut self, id: u64, undo: Vec<Command>) {
        self.hist.set_undo(id, undo);
    }

    /// Apply without touching the history; returns the inverse, worked out before the change.
    pub(crate) fn run(&mut self, cmd: &Command) -> Result<Vec<Command>, ApplyError> {
        let pos = plan_doc(&self.doc, cmd, &self.limits)?;
        let inverse = self.invert(cmd);
        apply_planned_doc(&mut self.doc, cmd, pos);
        self.version += 1;
        Ok(inverse)
    }
}
