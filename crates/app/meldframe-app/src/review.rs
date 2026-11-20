//! Review and checkpoints (E5.2, E5.3): the app keeps the ops the server confirmed since it loaded the file, so
//! a batch can be reverted and a checkpoint restored with the rules of `meldframe-revert`. Both come out as
//! commands of the person in one history group: one undo takes them back, and the log only grows. The record
//! and the plans are in `meldframe_app_rules::review`.

use crate::app::App;
use crate::document::refuse;
use crate::event::Event;
use crate::state::{Checkpoint, CheckpointKind};
use meldframe_app_parts::document::apply_all;
use meldframe_app_rules::review::{
    checkpoint_label, full_log, plan_restore, plan_revert, push_checkpoint,
};
use meldframe_protocol::Command;
use meldframe_revert::LoggedOp;
use meldframe_sync::SubmitError;

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
        if actor != self.replica.actor() && self.review.starts_run(actor, now_ms) {
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
        let id = self.review.next_checkpoint_id(&self.prefix);
        let list = &mut self.state.checkpoints;
        push_checkpoint(
            list,
            Checkpoint {
                id,
                seq,
                label,
                kind,
                actor: actor.map(String::from),
            },
        );
        let count = list.len();
        self.events.push(Event::CheckpointsChanged { count });
    }

    pub(crate) fn create_checkpoint(&mut self, label: String) -> Result<(), SubmitError> {
        let label = match checkpoint_label(label) {
            Ok(label) => label,
            Err(code) => return refuse(&mut self.events, code),
        };
        let seq = self.replica.seq();
        self.add_checkpoint(seq, label, CheckpointKind::Manual, None);
        Ok(())
    }

    pub(crate) fn revert_batch(
        &mut self,
        actor: &str,
        key_prefix: &str,
    ) -> Result<(), SubmitError> {
        let (log, replica) = (&self.review.log, &self.replica);
        let planned = plan_revert(log, actor, key_prefix, || {
            full_log(log, replica.actor(), replica.seq(), replica.unconfirmed())
        });
        let r = match planned {
            Ok(r) => r,
            Err(code) => return refuse(&mut self.events, code),
        };
        self.check_room(&r.commands)?;
        let seq = self.replica.seq();
        self.add_checkpoint(
            seq,
            String::new(),
            CheckpointKind::BeforeRevert,
            Some(actor),
        );
        apply_all(&mut self.replica, &r.commands)?;
        self.events.push(Event::Reverted {
            requested: r.commands.len(),
            skipped: r.skipped,
        });
        Ok(())
    }

    pub(crate) fn restore_checkpoint(&mut self, id: &str) -> Result<(), SubmitError> {
        let Some(cp) = self.state.checkpoints.iter().find(|c| c.id == id).cloned() else {
            return refuse(&mut self.events, "unknown_checkpoint");
        };
        let planned = plan_restore(&self.review.log, cp.seq, self.replica.nodes());
        let cmds = match planned {
            Ok(cmds) => cmds,
            Err(code) => return refuse(&mut self.events, code),
        };
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
