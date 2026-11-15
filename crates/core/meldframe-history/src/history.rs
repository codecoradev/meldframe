//! Undo and redo of the local actor. The steps and the stacks are in `meldframe-undo` (`History`);
//! here the commands of a step run against the tree, and the step for the opposite stack is worked out
//! again from the tree as it is then, so undo followed by redo leaves the tree as it was.

use crate::file_state::FileState;
use meldframe_core::{ApplyError, rebalance_commands};
use meldframe_protocol::Command;
use meldframe_undo::{Entry, Rec, compact};

impl FileState {
    pub fn can_undo(&self) -> bool {
        self.hist.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.hist.can_redo()
    }

    pub fn undo_depth(&self) -> usize {
        self.hist.undo_depth()
    }

    pub fn redo_depth(&self) -> usize {
        self.hist.redo_depth()
    }

    /// Start one history step that covers many commands (one gesture). A second call while a group is
    /// open does nothing.
    pub fn begin_group(&mut self) {
        self.hist.begin_group();
    }

    /// Close the open group. A group with no command makes no step.
    pub fn end_group(&mut self) {
        self.hist.end_group();
    }

    /// Give the children of `parent` fresh `pos` keys in their current order, as one history step (or part
    /// of the caller's open group). Use it when repeated inserts into one gap have made the keys long.
    /// Returns how many children were processed.
    pub fn rebalance(&mut self, parent: Option<&str>) -> Result<usize, ApplyError> {
        let cmds = rebalance_commands(&self.nodes, parent);
        // Inside a group the caller opened, the commands join it and the caller closes it.
        let opened = !self.hist.group_is_open();
        if opened {
            self.begin_group();
        }
        let done = cmds.iter().try_for_each(|c| self.apply(c));
        if opened {
            self.end_group();
        }
        done.map(|()| cmds.len())
    }

    /// The id of the rec made by the last `apply`.
    pub fn last_rec(&self) -> u64 {
        self.hist.last_rec()
    }

    /// Mark new recs as pending until `confirm_rec`. For a replica that waits for the server.
    pub fn set_optimistic(&mut self, on: bool) {
        self.hist.set_optimistic(on);
    }

    /// The op of this rec came back from the server.
    pub fn confirm_rec(&mut self, id: u64) {
        self.hist.confirm_rec(id);
    }

    /// The op of this rec will never take effect: drop its commands, and the steps left empty.
    pub fn forget_rec(&mut self, id: u64) {
        self.hist.forget_rec(id);
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
            id: self.hist.new_entry_id(),
            strikes: 0,
            recs: vec![Rec {
                id: self.hist.new_rec_id(),
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
        self.hist.end_group();
        while let Some(e) = self.hist.pop_undo() {
            if let Some((done, opposite)) = self.exec_entry(&e, true) {
                self.hist.settle(true, e, opposite);
                return Some(done);
            }
        }
        None
    }

    /// Redo the last undone step.
    pub fn redo(&mut self) -> Option<Vec<Command>> {
        self.hist.end_group();
        while let Some(e) = self.hist.pop_redo() {
            if let Some((done, opposite)) = self.exec_entry(&e, false) {
                self.hist.settle(false, e, opposite);
                return Some(done);
            }
        }
        None
    }

    /// What step `id` applies now that remote commands rewrote its original. A held step is made again
    /// from it, so it keeps no stale value and loses no field written before my edit.
    pub fn refresh_run(&mut self, id: u64) -> Option<Vec<Command>> {
        self.hist.refresh_run(id)
    }

    /// The id of the step that the last `undo` or `redo` ran, once (`None` outside optimistic mode).
    pub fn take_last_run(&mut self) -> Option<u64> {
        self.hist.take_last_run()
    }

    /// Every op of the step is confirmed: the copy of the original is not needed any more.
    pub fn finish_run(&mut self, id: u64) {
        self.hist.finish_run(id);
    }

    /// An op of the step was refused or dropped: put the history back (see
    /// `meldframe_undo::History::put_back_run` for the rules). `direct` is true when the server refused
    /// an op of this step itself, not a cascade or a rebase. Call it for the newest step first when
    /// several fall together.
    pub fn put_back_run(&mut self, id: u64, direct: bool) {
        if self.hist.put_back_run(id, direct) {
            self.version += 1;
        }
    }
}
