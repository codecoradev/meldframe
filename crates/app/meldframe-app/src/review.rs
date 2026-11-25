//! Review and restore (E5.2, E5.3): the app keeps the ops the server confirmed since it loaded the file, so a batch can be
//! reverted with the rules of `meldframe-revert`, and works out a restore from the file as the server had it at the
//! checkpoint. Both come out as commands of the person in one history group: one undo takes them back, and the log only
//! grows. Where the caller may make a checkpoint both wait for the server's row first (`checkpoints.rs`). The record and
//! the plans are in `meldframe_app_rules::review`.

use crate::app::App;
use crate::document::refuse;
use crate::effect::Effect;
use crate::event::Event;
use meldframe_app_model::CheckpointKind;
use meldframe_app_parts::document::apply_all;
use meldframe_app_rules::checkpoints::Waiting;
use meldframe_app_rules::review::{Revert, full_log, plan_restore, plan_revert};
use meldframe_protocol::{Command, Node};
use meldframe_revert::LoggedOp;
use meldframe_sync::SubmitError;

impl App {
    /// A confirmed op, in the server's order. `actor` is the lane key (`crate::text::actor`, empty for an actor with no
    /// name) and the key is made safe here, so the log, the reducer and the batch rows agree on both. An op with no
    /// actor stays in the log, whose order a revert reads, and feeds no lane.
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
            key: key.map(crate::text::op_key).unwrap_or_default(),
            command: command.clone(),
        };
        self.review.log.push(op.clone());
        if !actor.is_empty() {
            self.feed.red.observe_op(&op, now_ms);
        }
    }

    /// The revert of a batch on the file as it is now, or the code of the refusal.
    fn revert_plan(&self, actor: &str, key_prefix: &str) -> Result<Revert, &'static str> {
        let (log, replica) = (&self.review.log, &self.replica);
        plan_revert(log, actor, key_prefix, || {
            full_log(log, replica.actor(), replica.seq(), replica.unconfirmed())
        })
    }

    pub(crate) fn revert_batch(
        &mut self,
        actor: &str,
        key_prefix: &str,
        fx: &mut Vec<Effect>,
    ) -> Result<(), SubmitError> {
        let r = match self.revert_plan(actor, key_prefix) {
            Ok(r) => r,
            Err(code) => return refuse(&mut self.events, code),
        };
        self.check_room(&r.commands)?;
        if !self.state.can_create_checkpoint {
            return self.apply_revert(r);
        }
        let waiting = |key| Waiting::Revert {
            key,
            actor: actor.to_string(),
            key_prefix: key_prefix.to_string(),
        };
        self.wait_for_post(CheckpointKind::BeforeRevert, Some(actor), waiting, fx)
    }

    /// The row before a revert is made: work the revert out again on the file as it is now, and apply it.
    pub(crate) fn go_on_revert(
        &mut self,
        actor: &str,
        key_prefix: &str,
    ) -> Result<(), SubmitError> {
        match self.revert_plan(actor, key_prefix) {
            Ok(r) => self
                .check_room(&r.commands)
                .and_then(|()| self.apply_revert(r)),
            Err(code) => refuse(&mut self.events, code),
        }
    }

    fn apply_revert(&mut self, r: Revert) -> Result<(), SubmitError> {
        apply_all(&mut self.replica, &r.commands)?;
        self.events.push(Event::Reverted {
            requested: r.commands.len(),
            skipped: r.skipped,
        });
        Ok(())
    }

    pub(crate) fn restore_checkpoint(
        &mut self,
        id: &str,
        fx: &mut Vec<Effect>,
    ) -> Result<(), SubmitError> {
        let Some(n) = self
            .state
            .checkpoints
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.n)
        else {
            return refuse(&mut self.events, "unknown_checkpoint");
        };
        // The state of a file at a checkpoint is read by the people who may make one.
        if !self.state.can_create_checkpoint {
            return refuse(&mut self.events, "forbidden");
        }
        if self.cps.waiting.is_some() {
            return refuse(&mut self.events, "checkpoint_pending");
        }
        self.cps.waiting = Some(Waiting::State { n, id: id.into() });
        fx.push(Effect::FetchCheckpointState { n });
        Ok(())
    }

    /// The file as it was at the checkpoint came: work the commands out on the current replica, then ask for the row
    /// that goes before them.
    pub(crate) fn restore_planned(&mut self, id: String, target: Vec<Node>, fx: &mut Vec<Effect>) {
        let planned = plan_restore(&target, self.replica.nodes());
        let planned = planned.map_err(String::from).and_then(|cmds| {
            let room = self.check_room(&cmds);
            room.map_err(|e| meldframe_app_parts::document::code(&e).to_string())
        });
        match planned {
            Err(code) => self.events.push(Event::Refused { code }),
            Ok(()) => {
                let waiting = |key| Waiting::Restore { key, id, target };
                drop(self.wait_for_post(CheckpointKind::BeforeRestore, None, waiting, fx));
            }
        }
    }

    /// The row before a restore is made: work the commands out again on the file as it is now, and apply them.
    pub(crate) fn go_on_restore(&mut self, id: &str, target: &[Node]) -> Result<(), SubmitError> {
        let cmds = match plan_restore(target, self.replica.nodes()) {
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
