//! The confirmed ops a client keeps for a revert or a restore: the file as of `base_seq` and the ops after it.
//!
//! The record is bounded. When it reaches [`MAX_LOG`] plus [`FOLD_CHUNK`] ops, the oldest ones are folded into
//! the base tree in one pass, down to [`MAX_LOG`]: a fold costs the size of the file once per chunk, not once per
//! op. A base that the ops do not replay on is not hidden: [`ConfirmedLog::diverged`] says so and the caller
//! refuses to work out a revert or a restore from it.

use crate::{LoggedOp, batch_index};
use meldframe_history::FileState;
use meldframe_protocol::Node;
use std::collections::HashSet;

/// The most ops kept after a fold.
pub const MAX_LOG: usize = 2_000;
/// How many ops the record may grow past [`MAX_LOG`] before it folds.
pub const FOLD_CHUNK: usize = 256;
/// The batches remembered as folded; past it the memory is cleared and a batch is judged by its first index.
const MAX_FOLDED: usize = 4_096;

#[derive(Debug, Default)]
pub struct ConfirmedLog {
    base: Vec<Node>,
    base_seq: u64,
    log: Vec<LoggedOp>,
    /// `(actor, key prefix)` of every batch with an op folded into the base.
    folded: HashSet<(String, String)>,
    folded_overflow: bool,
    diverged: bool,
}

impl ConfirmedLog {
    pub fn new(base: Vec<Node>, base_seq: u64) -> Self {
        ConfirmedLog {
            base,
            base_seq,
            ..Default::default()
        }
    }

    /// Start again from a loaded file: the ops kept so far no longer lead to it.
    pub fn reset(&mut self, base: Vec<Node>, base_seq: u64) {
        *self = ConfirmedLog::new(base, base_seq);
    }

    /// The tree as of [`base_seq`](Self::base_seq).
    pub fn base(&self) -> &[Node] {
        &self.base
    }

    pub fn base_seq(&self) -> u64 {
        self.base_seq
    }

    /// The confirmed ops after the base, oldest first.
    pub fn ops(&self) -> &[LoggedOp] {
        &self.log
    }

    /// True when a fold met an op that does not replay on the base: the record can no longer be trusted.
    pub fn diverged(&self) -> bool {
        self.diverged
    }

    /// Add the next confirmed op; fold the oldest ones into the base when the record is a chunk over its size.
    pub fn push(&mut self, op: LoggedOp) {
        self.log.push(op);
        if self.log.len() >= MAX_LOG + FOLD_CHUNK {
            self.fold(self.log.len() - MAX_LOG);
        }
    }

    fn fold(&mut self, count: usize) {
        let mut fs = FileState::from_nodes_unchecked(std::mem::take(&mut self.base));
        for op in self.log.drain(..count) {
            if fs.replay_pending(&op.command).is_err() {
                self.diverged = true;
            }
            self.base_seq = op.seq;
            if let Some((prefix, _)) = batch_index(&op.key) {
                self.folded.insert((op.actor, prefix.to_string()));
            }
        }
        self.base = fs.nodes().to_vec();
        if self.folded.len() > MAX_FOLDED {
            self.folded.clear();
            self.folded_overflow = true;
        }
    }

    /// True when some ops of the batch `{actor, key_prefix}` were folded into the base, so a revert from the
    /// ops that are left would take back only the end of it. After the memory of folded batches overflowed
    /// (an unusually long session) a batch whose first op in the record is not number 0 counts as folded too.
    pub fn batch_is_partial(&self, actor: &str, key_prefix: &str) -> bool {
        if self
            .folded
            .contains(&(actor.to_string(), key_prefix.to_string()))
        {
            return true;
        }
        self.folded_overflow
            && self
                .log
                .iter()
                .filter(|op| op.actor == actor)
                .filter_map(|op| batch_index(&op.key))
                .filter(|(p, _)| *p == key_prefix)
                .map(|(_, n)| n)
                .min()
                .is_some_and(|n| n > 0)
    }
}
