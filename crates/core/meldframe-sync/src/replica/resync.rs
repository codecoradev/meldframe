//! Loading the file again after `ReplicaError::Diverged` or a lost connection, without losing my pending ops.

use super::Replica;
use meldframe_check::LoadError;
use meldframe_history::FileState;
use meldframe_protocol::Node;
use std::collections::HashSet;

impl Replica {
    /// Start again from a new snapshot (`nodes` and its `seq`, checked like the one of `Replica::new`; a
    /// bad one is refused and the replica is left as it was). My pending ops stay, with their keys,
    /// kinds and dependencies, and are applied on top of the snapshot in order; the outbox holds them all
    /// again, to be sent once more (the same key makes a repeat harmless). `applied` holds keys of my
    /// ops that the server already applied, so the snapshot holds their result: they are done, not
    /// replayed. An op that does not apply any more is dropped with the ops built on it; their number is
    /// returned. The events between the old `seq` and the snapshot were not followed, so the undo history
    /// cannot be: it starts again, and each replayed op is one step of it. Corrections that were on their
    /// way are forgotten. Call it, send `take_outbox()`, then follow the stream from `seq`.
    pub fn resync(
        &mut self,
        nodes: Vec<Node>,
        seq: u64,
        applied: &[String],
    ) -> Result<usize, LoadError> {
        let mut view = FileState::try_from_nodes(nodes, self.limits.clone())?;
        view.set_optimistic(true);
        self.base = view.nodes().to_vec();
        self.view = view;
        self.seq = seq;
        self.ideal = None;
        self.run_left.clear();
        let done: HashSet<&str> = applied.iter().map(String::as_str).collect();
        let (mut dead, mut dropped) = (HashSet::new(), 0);
        for mut p in std::mem::take(&mut self.pending) {
            if done.contains(p.out.key.as_str()) {
                continue;
            }
            let live = !p.out.deps.iter().any(|d| dead.contains(d))
                && self.view.apply_keyed(&p.out.command, None).is_ok();
            if !live {
                dead.insert(p.out.key.clone());
                dropped += 1;
                continue;
            }
            (p.rec, p.run) = (Some(self.view.last_rec()), None);
            (p.intent, p.resent, p.ideal_cmds) = (None, false, None);
            self.pending.push_back(p);
        }
        self.outbox = self.pending.iter().map(|p| p.out.clone()).collect();
        Ok(dropped)
    }
}
