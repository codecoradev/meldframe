//! Undo and redo of one actor, without the tree. Each step keeps the commands that redo it and the
//! commands that undo it. When a step runs, the opposite step is worked out again from the tree as it
//! is then (by `FileState`), so undo followed by redo leaves the tree as it was.

use crate::runs::Run;
use meldframe_protocol::Command;

/// One command with its inverse; both lists are in the order they are applied.
#[derive(Debug, Clone)]
pub struct Rec {
    pub id: u64,
    /// The op of this rec has not come back yet: a remote command is ordered before it and does not
    /// rewrite it; its undo is worked out again on replay (a step made from it: `refresh_run`).
    pub pending: bool,
    pub do_: Vec<Command>,
    pub undo_: Vec<Command>,
}

/// One step of history: what one undo or redo moves over.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Goes up in creation order, so a stack is ordered by id (`put_back_run` relies on it).
    pub id: u64,
    pub recs: Vec<Rec>,
    /// How many times the server refused an op of this step outright (see `put_back_run`).
    pub strikes: u8,
}

/// Steps kept on the undo stack; the oldest is dropped beyond this.
pub(crate) const MAX_HISTORY: usize = 500;

/// The two stacks, the open group, the optimistic runs and the counters that number them.
#[derive(Debug, Clone, Default)]
pub struct History {
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

impl History {
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.group.as_ref().is_some_and(|g| !g.recs.is_empty())
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_depth(&self) -> usize {
        self.redo.len()
    }

    /// Record a command that was just applied. A new edit discards redo.
    pub fn record(&mut self, cmd: &Command, inverse: Vec<Command>, key: Option<&str>) {
        if !self.redo.is_empty() {
            self.redo_epoch += 1;
        }
        self.redo.clear();
        let rec = Rec {
            id: self.new_rec_id(),
            pending: self.optimistic,
            do_: vec![cmd.clone()],
            undo_: inverse,
        };
        if let Some(g) = self.group.as_mut() {
            g.recs.push(rec);
            self.merge_key = None;
        } else if let (Some(k), Some(top)) = (key, self.undo.last_mut())
            && self.merge_key.as_deref() == Some(k)
        {
            top.recs.push(rec);
        } else {
            let id = self.new_entry_id();
            self.push(Entry {
                id,
                recs: vec![rec],
                strikes: 0,
            });
            self.merge_key = key.map(String::from);
        }
    }

    /// Start one history step that covers many commands (one gesture). A second call while a group is
    /// open does nothing.
    pub fn begin_group(&mut self) {
        if self.group.is_none() {
            self.merge_key = None;
            let id = self.new_entry_id();
            self.group = Some(Entry {
                id,
                recs: Vec::new(),
                strikes: 0,
            });
        }
    }

    /// Close the open group. A group with no command makes no step.
    pub fn end_group(&mut self) {
        self.merge_key = None;
        if let Some(g) = self.group.take()
            && !g.recs.is_empty()
        {
            self.push(g);
        }
    }

    /// Whether a group is open.
    pub fn group_is_open(&self) -> bool {
        self.group.is_some()
    }

    /// A keyed edit no longer joins the step on top (a remote command may have changed or pruned it).
    pub fn close_merge(&mut self) {
        self.merge_key = None;
    }

    pub fn new_entry_id(&mut self) -> u64 {
        self.next_entry += 1;
        self.next_entry
    }

    pub fn new_rec_id(&mut self) -> u64 {
        self.next_rec += 1;
        self.next_rec
    }

    /// The id of the rec made by the last `apply`.
    pub fn last_rec(&self) -> u64 {
        self.next_rec
    }

    /// Mark new recs as pending until `confirm_rec`. For a replica that waits for the server.
    pub fn set_optimistic(&mut self, on: bool) {
        self.optimistic = on;
    }

    /// The op of this rec came back from the server.
    pub fn confirm_rec(&mut self, id: u64) {
        for r in self.recs_mut().filter(|r| r.id == id) {
            r.pending = false;
        }
    }

    /// The op of this rec will never take effect: drop its commands, and the steps left empty.
    pub fn forget_rec(&mut self, id: u64) {
        for r in self.recs_mut().filter(|r| r.id == id) {
            r.do_.clear();
            r.undo_.clear();
        }
        self.prune();
    }

    /// Give the rec this undo, so it holds the value that is right in the server's order.
    pub fn set_undo(&mut self, id: u64, undo: Vec<Command>) {
        for r in self.recs_mut().filter(|r| r.id == id) {
            r.undo_ = undo.clone();
        }
    }

    /// Every rec of every step.
    pub(crate) fn recs_mut(&mut self) -> impl Iterator<Item = &mut Rec> {
        self.entries_mut().flat_map(|e| e.recs.iter_mut())
    }

    /// Every step that can hold commands: both stacks and the open group.
    fn entries_mut(&mut self) -> impl Iterator<Item = &mut Entry> {
        self.undo
            .iter_mut()
            .chain(self.redo.iter_mut())
            .chain(self.group.iter_mut())
            .chain(self.runs.iter_mut().map(|m| &mut m.shadow))
    }

    /// Drop recs and steps that no longer hold a command.
    pub(crate) fn prune(&mut self) {
        for e in self.entries_mut() {
            e.recs.retain(|r| !(r.do_.is_empty() && r.undo_.is_empty()));
        }
        self.undo.retain(|e| !e.recs.is_empty());
        self.redo.retain(|e| !e.recs.is_empty());
    }

    fn push(&mut self, e: Entry) {
        self.undo.push(e);
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
        }
    }

    /// The step on top of the undo stack, taken off it.
    pub fn pop_undo(&mut self) -> Option<Entry> {
        self.undo.pop()
    }

    /// The step on top of the redo stack, taken off it.
    pub fn pop_redo(&mut self) -> Option<Entry> {
        self.redo.pop()
    }

    /// A step that was popped has run: `original` is the step as it was, `opposite` the one that took
    /// its place on the other stack (the redo stack after an undo, the undo stack after a redo).
    pub fn settle(&mut self, undoing: bool, original: Entry, opposite: Entry) {
        self.note_run(undoing, original, &opposite);
        if undoing {
            self.redo.push(opposite);
        } else {
            self.undo.push(opposite);
        }
    }
}
