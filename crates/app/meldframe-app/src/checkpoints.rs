//! The checkpoints of the file in the app layer (E5.3, ADR-0023): the list as the server last said it, the request the
//! app makes (the list). A checkpoint
//! changes only when the server has said so, never optimistically, and the app takes none itself. The rules are in
//! `meldframe_app_rules::checkpoints` and `meldframe-checkpoints`.

use crate::app::App;
use crate::effect::Effect;
use crate::event::Event;
use crate::state::Connection;
use meldframe_app_model::{Action, CheckpointRequest, CheckpointRow, CheckpointsStatus};
use meldframe_app_rules::checkpoints::{Taken, merge, take};

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

    /// The file was loaded again: a read of the list that was out will not come back.
    pub(crate) fn checkpoints_reset(&mut self) {
        self.cps.listing = false;
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

    /// The answers that change no node: the list, a row that was made, a failed request.
    pub(crate) fn checkpoint_action(&mut self, action: Action) {
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
            Action::CheckpointRequestFailed { request, code } => {
                self.checkpoint_failed(request, &code)
            }
            _ => {}
        }
    }

    fn checkpoint_failed(&mut self, request: CheckpointRequest, code: &str) {
        match request {
            CheckpointRequest::List {} => {
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
        }
    }
}
