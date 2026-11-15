//! Undo and redo steps that wait for the server (optimistic mode, used by a replica).
//!
//! When a step runs, the original step is kept as a shadow, rewritten against remote commands like any
//! other. If the server refuses the op of that step, `put_back_run` puts the history back where it
//! was, so the step can be tried again.

use crate::file_state::FileState;
use crate::history::{Entry, MAX_HISTORY, Rec};
use crate::patch_ops::compact;
use meldframe_protocol::Command;

/// One undo or redo step that has run but whose ops are not all confirmed.
#[derive(Debug, Clone)]
pub(crate) struct Run {
    id: u64,
    /// True when the step came off the undo stack (its result is on the redo stack).
    undoing: bool,
    /// The original step.
    pub(crate) shadow: Entry,
    /// The rec of the step that took its place on the opposite stack.
    rec: u64,
    /// `redo_epoch` when the step ran.
    epoch: u64,
}

impl FileState {
    /// Keep a copy of the step that just ran (optimistic mode only).
    pub(crate) fn note_run(&mut self, undoing: bool, original: Entry, produced: &Entry) {
        self.last_run = None;
        if !self.optimistic {
            return;
        }
        self.next_run += 1;
        self.runs.push(Run {
            id: self.next_run,
            undoing,
            shadow: original,
            rec: produced.recs[0].id,
            epoch: self.redo_epoch,
        });
        self.last_run = Some(self.next_run);
    }

    /// Recs of ops ordered after every remote command seen: a step whose original holds a pending (or
    /// held) rec. Remote commands skip them; `refresh_run` makes them again.
    pub(crate) fn held_recs(&self) -> Vec<u64> {
        let mut held = Vec::new();
        for m in &self.runs {
            let hit = |r: &Rec| r.pending || held.contains(&r.id);
            if m.shadow.recs.iter().any(hit) {
                held.push(m.rec);
            }
        }
        held
    }

    /// What step `id` applies now that remote commands rewrote its original. A held step is made again
    /// from it, so it keeps no stale value and loses no field written before my edit.
    pub fn refresh_run(&mut self, id: u64) -> Option<Vec<Command>> {
        let m = self.runs.iter().find(|m| m.id == id)?;
        let (rec, undoing, recs) = (m.rec, m.undoing, &m.shadow.recs);
        let undo = compact(recs.iter().rev().flat_map(|r| r.undo_.clone()).collect());
        let do_ = compact(recs.iter().flat_map(|r| r.do_.clone()).collect());
        if self.held_recs().contains(&rec) {
            for r in self.recs_mut().filter(|r| r.id == rec) {
                (r.do_, r.undo_) = (do_.clone(), undo.clone());
            }
        }
        Some(if undoing { undo } else { do_ })
    }

    /// The id of the step that the last `undo` or `redo` ran, once (`None` outside optimistic mode).
    pub fn take_last_run(&mut self) -> Option<u64> {
        self.last_run.take()
    }

    /// Every op of the step is confirmed: the copy of the original is not needed any more.
    pub fn finish_run(&mut self, id: u64) {
        self.runs.retain(|m| m.id != id);
    }

    /// An op of the step was refused or dropped: put the history back. The step that took its place is
    /// removed from the opposite stack; the original goes back to the stack it came from, ordered by id
    /// (below steps made after it).
    ///
    /// - A refused undo goes back to the undo stack, unless it was used by another redo that is still
    ///   alive (its replacement is gone from the redo stack and no new edit discarded it).
    /// - A refused redo goes back to the redo stack only if no new edit has discarded redo since.
    /// - A step whose recs are all empty (its edit was refused or rewritten away) does not come back.
    ///
    /// `direct` is true when the server refused an op of this step itself, not a cascade or a rebase.
    /// Such a step comes back once to be tried again; refused outright a second time it is dropped, so
    /// undo is never stuck on a step the server always refuses. Call it for the newest step first when
    /// several fall together.
    pub fn put_back_run(&mut self, id: u64, direct: bool) {
        let Some(i) = self.runs.iter().position(|m| m.id == id) else {
            return;
        };
        let mut m = self.runs.remove(i);
        m.shadow.strikes += u8::from(direct);
        let opposite = if m.undoing {
            &mut self.redo
        } else {
            &mut self.undo
        };
        let found = opposite
            .iter()
            .position(|e| e.recs.iter().any(|r| r.id == m.rec))
            .map(|k| opposite.remove(k))
            .is_some();
        let cleared = self.redo_epoch != m.epoch;
        let restore = if m.undoing {
            found || cleared
        } else {
            found && !cleared
        };
        let alive = m
            .shadow
            .recs
            .iter()
            .any(|r| !(r.do_.is_empty() && r.undo_.is_empty()));
        if restore && alive && m.shadow.strikes < 2 {
            let origin = if m.undoing {
                &mut self.undo
            } else {
                &mut self.redo
            };
            let at = origin
                .iter()
                .position(|e| e.id > m.shadow.id)
                .unwrap_or(origin.len());
            origin.insert(at, m.shadow);
            if m.undoing && origin.len() > MAX_HISTORY {
                origin.remove(0);
            }
        }
        self.version += 1;
    }
}
