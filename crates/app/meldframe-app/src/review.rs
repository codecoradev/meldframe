//! Review and restore (E5.2, E5.3): the app keeps the ops the server confirmed since it loaded the file, so a batch can be
//! reverted and a checkpoint restored with the rules of `meldframe-revert`. Both come out as commands of the person in
//! one history group: one undo takes them back, and the log only grows. The record and the plans are in
//! `meldframe_app_rules::review`; the checkpoints themselves are the server's (`checkpoints.rs`).

use crate::app::App;
use crate::document::refuse;
use crate::event::Event;
use meldframe_app_parts::document::apply_all;
use meldframe_app_rules::review::{full_log, plan_restore, plan_revert};
use meldframe_protocol::Command;
use meldframe_revert::LoggedOp;
use meldframe_sync::SubmitError;

impl App {
    /// A confirmed op, in the server's order.
    pub(crate) fn record_op(
        &mut self,
        seq: u64,
        actor: &str,
        key: Option<&str>,
        command: &Command,
        now_ms: u64,
    ) {
        let op = LoggedOp {
            seq,
            actor: actor.to_string(),
            key: key.unwrap_or_default().to_string(),
            command: command.clone(),
        };
        self.review.log.push(op.clone());
        self.feed.red.observe_op(&op, now_ms);
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
        apply_all(&mut self.replica, &r.commands)?;
        self.events.push(Event::Reverted {
            requested: r.commands.len(),
            skipped: r.skipped,
        });
        Ok(())
    }

    pub(crate) fn restore_checkpoint(&mut self, id: &str) -> Result<(), SubmitError> {
        let Some(seq) = self
            .state
            .checkpoints
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.seq)
        else {
            return refuse(&mut self.events, "unknown_checkpoint");
        };
        let planned = plan_restore(&self.review.log, seq, self.replica.nodes());
        let cmds = match planned {
            Ok(cmds) => cmds,
            Err(code) => return refuse(&mut self.events, code),
        };
        self.check_room(&cmds)?;
        apply_all(&mut self.replica, &cmds)?;
        self.events.push(Event::Restored {
            id: id.to_string(),
            requested: cmds.len(),
        });
        Ok(())
    }
}
