//! Undo and redo of the local actor. Each step keeps the commands that redo it and the commands that
//! undo it. When a step runs, the opposite step is worked out again from the tree as it is then, so
//! undo followed by redo leaves the tree as it was.

use crate::file_state::FileState;
use meldframe_protocol::Command;

/// One command with its inverse; both lists are in the order they are applied.
#[derive(Debug, Clone)]
pub(crate) struct Rec {
    pub(crate) do_: Vec<Command>,
    pub(crate) undo_: Vec<Command>,
}

/// One step of history: what one undo or redo moves over.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub(crate) recs: Vec<Rec>,
}

/// Steps kept on the undo stack; the oldest is dropped beyond this.
const MAX_HISTORY: usize = 500;

impl FileState {
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
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
    pub(crate) fn record(&mut self, cmd: &Command, inverse: Vec<Command>) {
        self.redo.clear();
        let rec = Rec {
            do_: vec![cmd.clone()],
            undo_: inverse,
        };
        self.push(Entry { recs: vec![rec] });
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
        Some((
            done,
            Entry {
                recs: vec![Rec { do_, undo_ }],
            },
        ))
    }

    /// Undo the last step. Returns the commands that were applied, or `None` when there is nothing to
    /// undo. A step that can no longer apply anything is dropped and the one below it is tried.
    pub fn undo(&mut self) -> Option<Vec<Command>> {
        while let Some(e) = self.undo.pop() {
            if let Some((done, opposite)) = self.exec_entry(&e, true) {
                self.redo.push(opposite);
                return Some(done);
            }
        }
        None
    }

    /// Redo the last undone step.
    pub fn redo(&mut self) -> Option<Vec<Command>> {
        while let Some(e) = self.redo.pop() {
            if let Some((done, opposite)) = self.exec_entry(&e, false) {
                self.undo.push(opposite);
                return Some(done);
            }
        }
        None
    }
}
