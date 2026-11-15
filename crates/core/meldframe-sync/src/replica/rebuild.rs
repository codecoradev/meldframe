//! Building the view again after a remote op or a refusal, and the corrections that go with it.

use super::{OpKind, Replica, Source, footprint};
use crate::correction::{field_diff, inverse_in, without_fields};
use meldframe_core::apply_command;
use meldframe_protocol::{Command, Node};
use std::collections::HashSet;

impl Replica {
    /// Build the view again: start from `base` and apply the pending ops one by one. `dead` holds keys
    /// of ops that are known to be gone (refused by the server). A pending op that names a dead key is
    /// dropped without a try, and its own key becomes dead (the server does the same: it refuses an op
    /// whose dependency was not applied). An op that no longer applies is dropped too, with its history.
    /// Pending ops are in creation order and deps point to older ops, so one pass is enough. Returns
    /// how many were dropped. `direct_run` is the step whose op the server refused itself; the steps
    /// of the ops dropped here join it, and the history of all of them is put back.
    pub(super) fn rebuild(
        &mut self,
        mut dead: HashSet<String>,
        direct_run: Option<u64>,
        remote: Option<&Command>,
        correct: bool,
    ) -> usize {
        let mut dead_runs: Vec<u64> = direct_run.into_iter().collect();
        self.view.reset_nodes(self.base.clone());
        // What the rebuilt steps would write, to set right the stale ops that are already on their way.
        // Without a step or a correction in the queue the view is the answer: nothing is built.
        let track = self.ideal.is_some()
            || self
                .pending
                .iter()
                .any(|p| p.run.is_some() || p.is_correction());
        let mut want = track.then(|| self.ideal.clone().unwrap_or_else(|| self.base.clone()));
        let mut seen = HashSet::new();
        let mut dropped = 0;
        for mut p in std::mem::take(&mut self.pending) {
            let cmd = &p.out.command;
            let alive = !p.out.deps.iter().any(|d| dead.contains(d))
                && self.view.replay_pending(cmd).is_ok_and(|inverse| {
                    if let Some(r) = p.rec {
                        // The undo of an edit is worked out in `want`, not in a view that holds stale ops.
                        let own = want.as_deref().and_then(|w| inverse_in(w, cmd));
                        self.view.set_undo(r, own.unwrap_or(inverse));
                    }
                    true
                });
            if alive {
                // A step counts once, at its first op; a correction is not part of any step.
                let cmds = match p.run {
                    Some(m) if seen.insert(m) => self.view.refresh_run(m).unwrap_or_default(),
                    None if p.out.kind == OpKind::Edit => vec![cmd.clone()],
                    // A correction counts, minus what a remote op wrote (it is ordered first).
                    None => {
                        if let Some(r) = remote {
                            p.intent = p.intent.take().and_then(|i| without_fields(&i, r));
                        }
                        p.intent.iter().cloned().collect()
                    }
                    _ => vec![],
                };
                if let Some(w) = want.as_mut() {
                    cmds.iter()
                        .for_each(|c| drop(apply_command(w, c, &self.limits)));
                    p.ideal_cmds = Some(cmds);
                }
                self.pending.push_back(p);
                continue;
            }
            dead.insert(p.out.key.clone());
            if let Some(r) = p.rec {
                self.view.forget_rec(r);
            }
            if let Some(m) = p.run.filter(|m| !dead_runs.contains(m)) {
                dead_runs.push(m);
            }
            self.outbox.retain(|o| o.key != p.out.key);
            dropped += 1;
        }
        self.put_back_runs(dead_runs, direct_run);
        // An op refused or dropped may have been part of a step whose first op `ideal` already took in
        // whole, and `want` took the rest: what the step would have added never happened. A refusal of a
        // stale op always drops its correction (it depends on it), so this covers it. `ideal` goes, and the
        // stale value that a confirmed stale op left in `base` is forgotten.
        if dropped > 0 || self.pending.is_empty() {
            self.ideal = None;
            want = want.filter(|_| dropped == 0);
        }
        if let Some(w) = want.filter(|_| correct) {
            self.correct(&w);
        }
        dropped
    }

    /// My op `cmd` is about to be confirmed (it is the first pending op): `ideal` takes what it adds.
    /// Once the queue is empty nothing is owed and `ideal` goes.
    pub(super) fn advance_ideal(&mut self, cmd: &Command) {
        let front = self.pending.front();
        if self.pending.len() < 2
            || (self.ideal.is_none() && front.is_none_or(|p| p.ideal_cmds.is_none()))
        {
            self.ideal = None;
            return;
        }
        let own = [cmd.clone()];
        let cmds = front.and_then(|p| p.ideal_cmds.as_deref()).unwrap_or(&own);
        let mut w = self.ideal.take().unwrap_or_else(|| self.base.clone());
        cmds.iter()
            .for_each(|c| drop(apply_command(&mut w, c, &self.limits)));
        self.ideal = Some(w);
    }

    /// Send what sets right the fields where the view, which holds stale undo or redo ops that are on
    /// their way, differs from `want` (see `correction`).
    fn correct(&mut self, want: &[Node]) {
        for c in field_diff(self.view.nodes(), want) {
            let foot = footprint(self.view.nodes(), &c);
            if self.view.replay_pending(&c).is_ok() {
                self.enqueue(
                    c.clone(),
                    OpKind::Undo,
                    Source::default(),
                    foot,
                    self.pending.len(),
                    None,
                );
                if let Some(last) = self.pending.back_mut() {
                    last.intent = Some(c);
                }
            }
        }
    }
}
