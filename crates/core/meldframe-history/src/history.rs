//! Undo and redo of the local actor. Each step keeps the commands that redo it and the commands that
//! undo it. When a step runs, the opposite step is worked out again from the tree as it is then, so
//! undo followed by redo leaves the tree as it was.

use crate::file_state::FileState;
use crate::patch_ops::compact;
use meldframe_core::{ApplyError, rebalance_commands};
use meldframe_protocol::Command;

/// One command with its inverse; both lists are in the order they are applied.
#[derive(Debug, Clone)]
pub(crate) struct Rec {
    pub(crate) id: u64,
    /// The op of this rec has not come back from the server yet. A remote command is ordered before
    /// it, so the rec is not rewritten against that command; its undo is worked out again on replay.
    pub(crate) pending: bool,
    pub(crate) do_: Vec<Command>,
    pub(crate) undo_: Vec<Command>,
}

/// One step of history: what one undo or redo moves over.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    /// Goes up in creation order, so a stack is ordered by id (`put_back_run` relies on it).
    pub(crate) id: u64,
    pub(crate) recs: Vec<Rec>,
    /// How many times the server refused an op of this step outright (see `put_back_run`).
    pub(crate) strikes: u8,
}

/// Steps kept on the undo stack; the oldest is dropped beyond this.
pub(crate) const MAX_HISTORY: usize = 500;

impl FileState {
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
    pub(crate) fn record(&mut self, cmd: &Command, inverse: Vec<Command>, key: Option<&str>) {
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

    /// Give the children of `parent` fresh `pos` keys in their current order, as one history step (or part
    /// of the caller's open group). Use it
    /// when repeated inserts into one gap have made the keys long. Returns how many children were
    /// processed.
    pub fn rebalance(&mut self, parent: Option<&str>) -> Result<usize, ApplyError> {
        let cmds = rebalance_commands(&self.nodes, parent);
        // Inside a group the caller opened, the commands join it and the caller closes it.
        let opened = self.group.is_none();
        if opened {
            self.begin_group();
        }
        for c in &cmds {
            if let Err(e) = self.apply(c) {
                if opened {
                    self.end_group();
                }
                return Err(e);
            }
        }
        if opened {
            self.end_group();
        }
        Ok(cmds.len())
    }

    fn new_entry_id(&mut self) -> u64 {
        self.next_entry += 1;
        self.next_entry
    }

    fn new_rec_id(&mut self) -> u64 {
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

    /// Every rec of every step.
    pub(crate) fn recs_mut(&mut self) -> impl Iterator<Item = &mut Rec> {
        self.entries_mut().flat_map(|e| e.recs.iter_mut())
    }

    /// Every step that can hold commands: both stacks and the open group.
    pub(crate) fn entries_mut(&mut self) -> impl Iterator<Item = &mut Entry> {
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

    /// Run one step (undo when `undoing`). A command that no longer applies is skipped, because the tree
    /// has moved on. Returns the commands that were applied and the step for the opposite stack, or
    /// `None` when nothing could be applied.
    fn exec_entry(&mut self, e: &Entry, undoing: bool) -> Option<(Vec<Command>, Entry)> {
        let cmds: Vec<Command> = if undoing {
            e.recs.iter().rev().flat_map(|r| r.undo_.clone()).collect()
        } else {
            e.recs.iter().flat_map(|r| r.do_.clone()).collect()
        };
        let cmds = compact(cmds);
        let mut applied: Vec<(Command, Vec<Command>)> = Vec::new();
        for c in cmds {
            if let Ok(inv) = self.run(&c) {
                applied.push((c, inv));
            }
        }
        if applied.is_empty() {
            return None;
        }
        let opposite: Vec<Command> = applied.iter().rev().flat_map(|(_, i)| i.clone()).collect();
        let done: Vec<Command> = applied.into_iter().map(|(c, _)| c).collect();
        let (do_, undo_) = if undoing {
            (opposite, done.clone())
        } else {
            (done.clone(), opposite)
        };
        let step = Entry {
            id: self.new_entry_id(),
            strikes: 0,
            recs: vec![Rec {
                id: self.new_rec_id(),
                pending: false,
                do_,
                undo_,
            }],
        };
        Some((done, step))
    }

    /// Undo the last step. Returns the commands that were applied, or `None` when there is nothing to
    /// undo. A step that can no longer apply anything is dropped and the one below it is tried.
    pub fn undo(&mut self) -> Option<Vec<Command>> {
        self.end_group();
        while let Some(e) = self.undo.pop() {
            if let Some((done, opposite)) = self.exec_entry(&e, true) {
                self.note_run(true, e, opposite.id);
                self.redo.push(opposite);
                return Some(done);
            }
        }
        None
    }

    /// Redo the last undone step.
    pub fn redo(&mut self) -> Option<Vec<Command>> {
        self.merge_key = None;
        self.end_group();
        while let Some(e) = self.redo.pop() {
            if let Some((done, opposite)) = self.exec_entry(&e, false) {
                self.note_run(false, e, opposite.id);
                self.undo.push(opposite);
                return Some(done);
            }
        }
        None
    }
}
