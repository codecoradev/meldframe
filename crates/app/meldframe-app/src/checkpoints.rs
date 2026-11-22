//! The checkpoints of the file in the app layer (E5.3, ADR-0023): the list as the server last said it, the requests the
//! app makes (the list, a new row, the file at a checkpoint) and what a revert or a restore waits for. A checkpoint
//! changes only when the server has said so, never optimistically, and the app takes none itself. The rules are in
//! `meldframe_app_rules::checkpoints` and `meldframe-checkpoints`.

use crate::app::App;
use crate::document::refuse;
use crate::effect::Effect;
use crate::event::Event;
use crate::state::Connection;
use meldframe_app_model::{
    Action, CheckpointKind, CheckpointRequest, CheckpointRow, CheckpointsStatus,
};
use meldframe_app_rules::checkpoints::{Taken, Waiting, merge, take};
use meldframe_checkpoints::Label;
use meldframe_sync::SubmitError;

impl App {
    fn say(&mut self, code: &str) {
        self.events.push(Event::Refused { code: code.into() });
    }

    /// Ask for the list, only while live and not while a read of it is out.
    pub(crate) fn fetch_checkpoints(&mut self, fx: &mut Vec<Effect>) {
        if self.state.connection == Connection::Live && !self.cps.listing {
            self.cps.listing = true;
            fx.push(Effect::FetchCheckpoints {});
        }
    }

    /// After a subscribe or a resync a frame may have been lost: read the list again, even when a read was out.
    pub(crate) fn checkpoints_connected(&mut self, fx: &mut Vec<Effect>) {
        self.cps.listing = false;
        self.fetch_checkpoints(fx);
    }

    /// The file was loaded again: what was waited for will not come back.
    pub(crate) fn checkpoints_reset(&mut self) {
        self.cps.listing = false;
        if self.cps.waiting.take().is_some() {
            self.say("checkpoint_failed");
        }
    }

    /// Ask the server for a row at the file the person sees and hold what the app wants done when it is made.
    pub(crate) fn wait_for_post(
        &mut self,
        kind: CheckpointKind,
        about: Option<&str>,
        waiting: impl FnOnce(String) -> Waiting,
        fx: &mut Vec<Effect>,
    ) -> Result<(), SubmitError> {
        if self.cps.waiting.is_some() {
            return refuse(&mut self.events, "checkpoint_pending");
        }
        let key = self.cps.next_key(&self.prefix);
        let about = about.map(String::from);
        let seq = self.replica.seq();
        fx.push(Effect::PostCheckpoint {
            key: key.clone(),
            kind,
            label: None,
            about,
            seq,
        });
        self.cps.waiting = Some(waiting(key));
        Ok(())
    }

    pub(crate) fn create_checkpoint(
        &mut self,
        label: &str,
        fx: &mut Vec<Effect>,
    ) -> Result<(), SubmitError> {
        let label = match Label::parse(label) {
            Ok(label) => label.map(|l| l.as_str().to_owned()),
            Err(e) => return refuse(&mut self.events, e.code()),
        };
        if !self.state.can_create_checkpoint {
            return refuse(&mut self.events, "forbidden");
        }
        let key = self.cps.next_key(&self.prefix);
        let seq = self.replica.seq();
        let kind = CheckpointKind::Manual;
        fx.push(Effect::PostCheckpoint {
            key,
            kind,
            label,
            about: None,
            seq,
        });
        Ok(())
    }

    /// A row joined the list: say so, unless it changed nothing.
    fn join(&mut self, row: CheckpointRow) -> Taken {
        let taken = take(&mut self.state.checkpoints, row);
        if taken != Taken::Same {
            let count = self.state.checkpoints.len();
            self.events.push(Event::CheckpointsChanged { count });
        }
        taken
    }

    pub(crate) fn checkpoint_frame(&mut self, row: CheckpointRow, fx: &mut Vec<Effect>) {
        if self.join(row) == Taken::Gap {
            self.fetch_checkpoints(fx);
        }
    }

    /// The row asked for with `key` is made: it joins the list, and the revert or restore that waited for it goes on.
    pub(crate) fn checkpoint_posted(
        &mut self,
        key: &str,
        row: CheckpointRow,
    ) -> Result<(), SubmitError> {
        self.join(row);
        let mine = match &self.cps.waiting {
            Some(Waiting::Revert { key: k, .. } | Waiting::Restore { key: k, .. }) => k == key,
            _ => false,
        };
        match mine.then(|| self.cps.waiting.take()).flatten() {
            Some(Waiting::Revert {
                actor, key_prefix, ..
            }) => self.go_on_revert(&actor, &key_prefix),
            Some(Waiting::Restore { id, target, .. }) => self.go_on_restore(&id, &target),
            _ => Ok(()),
        }
    }

    /// The answers that change no node: the list, the file at a checkpoint, a failed request.
    pub(crate) fn checkpoint_action(&mut self, action: Action, fx: &mut Vec<Effect>) {
        match action {
            Action::CheckpointsLoaded {
                checkpoints,
                can_create,
            } => {
                self.cps.listing = false;
                let list = merge(&self.state.checkpoints, checkpoints);
                if list != self.state.checkpoints {
                    let count = list.len();
                    self.state.checkpoints = list;
                    self.events.push(Event::CheckpointsChanged { count });
                }
                self.state.can_create_checkpoint = can_create;
                self.state.checkpoints_status = CheckpointsStatus::Loaded;
            }
            Action::CheckpointStateLoaded { n, seq, nodes } => {
                let Some(Waiting::State { n: asked, id }) = self.cps.waiting.clone() else {
                    return;
                };
                if asked != n {
                    return;
                }
                self.cps.waiting = None;
                let held = self.state.checkpoints.iter().find(|c| c.id == id);
                match held.map(|c| c.seq) {
                    None => self.say("unknown_checkpoint"),
                    Some(at) if at != seq => self.say("log_mismatch"),
                    Some(_) => self.restore_planned(id, nodes, fx),
                }
            }
            Action::CheckpointRequestFailed { request, code } => {
                self.checkpoint_failed(request, &code)
            }
            _ => {}
        }
    }

    fn checkpoint_failed(&mut self, request: CheckpointRequest, code: &str) {
        match (request, &self.cps.waiting) {
            (CheckpointRequest::List {}, _) => {
                self.cps.listing = false;
                // A server that keeps no checkpoints answers the route with not found: nothing to say about it.
                if code != "not_found" {
                    return self.say(code);
                }
                self.state.can_create_checkpoint = false;
                self.state.checkpoints_status = CheckpointsStatus::Absent;
                if !self.state.checkpoints.is_empty() {
                    self.state.checkpoints.clear();
                    self.events.push(Event::CheckpointsChanged { count: 0 });
                }
            }
            (
                CheckpointRequest::Post { key },
                Some(Waiting::Revert { key: k, .. } | Waiting::Restore { key: k, .. }),
            ) if *k == key => {
                self.cps.waiting = None;
                self.say("checkpoint_failed");
            }
            (CheckpointRequest::Post { .. }, _) => self.say(code),
            (CheckpointRequest::State { n }, Some(Waiting::State { n: asked, .. }))
                if *asked == n =>
            {
                self.cps.waiting = None;
                self.say(code);
            }
            (CheckpointRequest::State { .. }, _) => {}
        }
    }
}
