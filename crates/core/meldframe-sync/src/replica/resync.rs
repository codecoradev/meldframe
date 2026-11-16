//! Loading the file again after `ReplicaError::Diverged` or a lost connection, without losing my pending ops.

use super::Replica;
use crate::types::{OpKind, ResyncError};
use meldframe_history::FileState;
use meldframe_protocol::Node;
use std::collections::HashSet;

impl Replica {
    /// Start again from a new snapshot (`nodes` and its `seq`, checked like the one of `Replica::new`). A
    /// bad one, or one older than the events already applied, is refused and the replica is left as it
    /// was. My pending ops stay, with their keys, kinds and dependencies, and are applied on top of the
    /// snapshot in order; the outbox holds them all again. `applied` holds keys of my ops that the
    /// server already applied, so the snapshot holds their result: they are done, not replayed. The
    /// caller must pass every key whose answer it holds. An op the snapshot already holds but that is
    /// missing from `applied` is replayed: it may fail (an `AddNode` of an id that exists is a duplicate)
    /// and is then dropped with the ops built on it, such as its undo; if it replays, `Replica::on_ack`
    /// drains it when the server answers its repeated send. An op that does not apply any more is dropped with the ops built on it; their
    /// number is returned. The events between the old `seq` and the snapshot were not followed, so the
    /// undo history starts empty and no replayed op is a step of it: a pending undo or redo op stays an
    /// op of its kind, a correction stays one. Call it, send `take_outbox()` and feed the answers to
    /// `on_ack`, then follow the stream from `seq`.
    pub fn resync(
        &mut self,
        nodes: Vec<Node>,
        seq: u64,
        applied: &[String],
    ) -> Result<usize, ResyncError> {
        if seq < self.seq {
            return Err(ResyncError::Behind {
                have: self.seq,
                got: seq,
            });
        }
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
                && self.view.replay_pending(&p.out.command).is_ok();
            if !live {
                dead.insert(p.out.key.clone());
                dropped += 1;
                continue;
            }
            (p.rec, p.run, p.resent, p.ideal_cmds) = (None, None, false, None);
            if p.out.kind != OpKind::Edit {
                // What the op still sets in the model, as for a correction (see `correction`).
                p.intent = p.intent.take().or_else(|| Some(p.out.command.clone()));
            }
            self.pending.push_back(p);
        }
        self.outbox = self.pending.iter().map(|p| p.out.clone()).collect();
        Ok(dropped)
    }
}
