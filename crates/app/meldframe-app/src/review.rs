//! Review and checkpoints (E5.2, E5.3): the app keeps the ops the server confirmed since it loaded the file, so
//! a batch can be reverted and a checkpoint restored with the rules of `meldframe-revert`. Both come out as
//! commands of the person in one history group: one undo takes them back, and the log only grows.

use crate::app::App;
use crate::document::{apply_all, refuse};
use crate::event::{Event, RevertSkip, RevertSkipReason};
use crate::state::{Checkpoint, CheckpointKind};
use meldframe_protocol::{Command, Node};
use meldframe_revert::{
    Batch, ConfirmedLog, LoggedOp, RevertError, RunDetector, SkipReason, restore_commands,
    revert_batch, state_at,
};
use meldframe_sync::SubmitError;

/// A run ends after this long without an op of its actor (5 minutes).
const RUN_GAP_MS: u64 = 300_000;
/// The checkpoints kept; past it the oldest one taken on its own is dropped, and a saved one only when none is left.
const MAX_CHECKPOINTS: usize = 100;
/// The longest label of a saved checkpoint, in characters.
const MAX_LABEL: usize = 128;

/// The confirmed ops since the file was loaded (a bounded record), the run detector and the checkpoint counter.
#[derive(Debug)]
pub(crate) struct Review {
    pub(crate) log: ConfirmedLog,
    runs: RunDetector,
    next_checkpoint: u64,
}

impl Review {
    pub(crate) fn new(base: Vec<Node>, base_seq: u64) -> Self {
        Review {
            log: ConfirmedLog::new(base, base_seq),
            runs: RunDetector::new(RUN_GAP_MS),
            next_checkpoint: 0,
        }
    }

    /// Start again from a loaded file: the ops kept so far no longer lead to it.
    pub(crate) fn reset(&mut self, base: Vec<Node>, base_seq: u64) {
        self.log.reset(base, base_seq);
    }
}

impl App {
    /// A confirmed op, in the server's order. The first op of another actor's run takes a checkpoint of the file
    /// as it was just before.
    pub(crate) fn record_op(
        &mut self,
        seq: u64,
        actor: &str,
        key: Option<&str>,
        command: &Command,
        now_ms: u64,
    ) {
        if actor != self.replica.actor() && self.review.runs.observe(actor, now_ms) {
            self.add_checkpoint(
                seq.saturating_sub(1),
                String::new(),
                CheckpointKind::OtherActorRun,
                Some(actor),
            );
        }
        let before = self.review.log.base_seq();
        let op = LoggedOp {
            seq,
            actor: actor.to_string(),
            key: key.unwrap_or_default().to_string(),
            command: command.clone(),
        };
        self.review.log.push(op.clone());
        self.feed.red.observe_op(&op, now_ms);
        if self.review.log.base_seq() != before {
            self.prune_checkpoints();
        }
    }

    /// Drop the checkpoints older than the file the record starts from: they can no longer be restored.
    pub(crate) fn prune_checkpoints(&mut self) {
        let from = self.review.log.base_seq();
        let before = self.state.checkpoints.len();
        self.state.checkpoints.retain(|c| c.seq >= from);
        let count = self.state.checkpoints.len();
        if count != before {
            self.events.push(Event::CheckpointsChanged { count });
        }
    }

    fn add_checkpoint(
        &mut self,
        seq: u64,
        label: String,
        kind: CheckpointKind,
        actor: Option<&str>,
    ) {
        self.review.next_checkpoint += 1;
        let id = format!("{}-cp{}", self.prefix, self.review.next_checkpoint);
        let list = &mut self.state.checkpoints;
        list.push(Checkpoint {
            id,
            seq,
            label,
            kind,
            actor: actor.map(String::from),
        });
        if list.len() > MAX_CHECKPOINTS {
            // The oldest one the app took, not the one just added; a saved one only when all are saved.
            let at = list[..list.len() - 1]
                .iter()
                .position(|c| c.kind != CheckpointKind::Manual);
            list.remove(at.unwrap_or(0));
        }
        let count = list.len();
        self.events.push(Event::CheckpointsChanged { count });
    }

    /// The confirmed ops, then the person's own unconfirmed ones as if the server numbered them next.
    fn full_log(&self) -> Vec<LoggedOp> {
        let mut log = self.review.log.ops().to_vec();
        let me = self.replica.actor();
        let next = self.replica.seq() + 1;
        for (i, o) in self.replica.unconfirmed().into_iter().enumerate() {
            log.push(LoggedOp {
                seq: next + i as u64,
                actor: me.to_string(),
                key: o.key,
                command: o.command,
            });
        }
        log
    }

    pub(crate) fn create_checkpoint(&mut self, label: String) -> Result<(), SubmitError> {
        let label = label.trim().to_string();
        if label.chars().count() > MAX_LABEL {
            return refuse(&mut self.events, "limit");
        }
        let seq = self.replica.seq();
        self.add_checkpoint(seq, label, CheckpointKind::Manual, None);
        Ok(())
    }

    pub(crate) fn revert_batch(
        &mut self,
        actor: &str,
        key_prefix: &str,
    ) -> Result<(), SubmitError> {
        let batch = Batch { actor, key_prefix };
        if self.review.log.diverged() {
            return refuse(&mut self.events, "log_mismatch");
        }
        if self.review.log.batch_is_partial(actor, key_prefix) {
            return refuse(&mut self.events, "batch_incomplete");
        }
        let r = match revert_batch(self.review.log.base(), &self.full_log(), batch) {
            Ok(r) => r,
            Err(RevertError::NoSuchBatch) => return refuse(&mut self.events, "unknown_batch"),
            Err(RevertError::LogDoesNotReplay { .. }) => {
                return refuse(&mut self.events, "log_mismatch");
            }
        };
        if r.commands.is_empty() {
            return refuse(&mut self.events, "nothing_to_revert");
        }
        self.check_room(&r.commands)?;
        let seq = self.replica.seq();
        self.add_checkpoint(
            seq,
            String::new(),
            CheckpointKind::BeforeRevert,
            Some(actor),
        );
        apply_all(&mut self.replica, &r.commands)?;
        let skipped = r
            .skipped
            .into_iter()
            .map(|s| RevertSkip {
                node: s.node,
                what: s.what,
                reason: match s.reason {
                    SkipReason::ChangedSince => RevertSkipReason::ChangedSince,
                    SkipReason::NotApplicable => RevertSkipReason::NotApplicable,
                },
            })
            .collect();
        self.events.push(Event::Reverted {
            requested: r.commands.len(),
            skipped,
        });
        Ok(())
    }

    pub(crate) fn restore_checkpoint(&mut self, id: &str) -> Result<(), SubmitError> {
        let Some(cp) = self.state.checkpoints.iter().find(|c| c.id == id).cloned() else {
            return refuse(&mut self.events, "unknown_checkpoint");
        };
        // A checkpoint older than the record is pruned when the record moves, so this is a guard.
        if cp.seq < self.review.log.base_seq() {
            return refuse(&mut self.events, "not_in_log");
        }
        if self.review.log.diverged() {
            return refuse(&mut self.events, "log_mismatch");
        }
        let target = state_at(self.review.log.base(), self.review.log.ops(), cp.seq);
        let cmds = target
            .ok()
            .and_then(|t| restore_commands(&t, self.replica.nodes()).ok());
        let Some(cmds) = cmds else {
            return refuse(&mut self.events, "log_mismatch");
        };
        if cmds.is_empty() {
            return refuse(&mut self.events, "nothing_to_restore");
        }
        self.check_room(&cmds)?;
        let seq = self.replica.seq();
        self.add_checkpoint(seq, String::new(), CheckpointKind::BeforeRestore, None);
        apply_all(&mut self.replica, &cmds)?;
        self.events.push(Event::Restored {
            id: id.to_string(),
            requested: cmds.len(),
        });
        Ok(())
    }
}
