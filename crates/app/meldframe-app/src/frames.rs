//! The frames the server sends: heartbeat, resync, batch and touch, acks, refusals and live events.

use crate::action::Frame;
use crate::app::App;
use crate::effect::Effect;
use crate::event::Event;
use crate::state::Connection;
use meldframe_app_parts::touched::{render_set, touched};
use meldframe_app_rules::level::Level;
use meldframe_protocol::Command;
use meldframe_sync::{EventOutcome, ReplicaError};

impl App {
    pub(crate) fn frame(&mut self, now_ms: u64, frame: Frame, fx: &mut Vec<Effect>) {
        self.heard(now_ms);
        match frame {
            Frame::Heartbeat {} => {}
            Frame::Resync { after_seq, .. } => {
                // A marker ahead of the replica (a stale one) must not skip ops it has not got.
                self.resubscribe(after_seq.min(self.replica.seq()), fx);
                self.fetched(|f, _| f.connected(now_ms), fx);
                self.checkpoints_connected(fx);
            }
            Frame::Checkpoint { checkpoint } => self.checkpoint_frame(checkpoint, fx),
            Frame::Batch { .. } | Frame::Touch { .. } => self.activity_frame(now_ms, frame, fx),
            Frame::Rejected {
                key,
                code,
                retryable,
            } => {
                if self.refused(now_ms, key, code, retryable, fx) {
                    self.changed(self.replica.actor().to_string(), true, Vec::new(), fx);
                }
            }
            Frame::Acked { key, seq } => {
                if let Some(dropped) = self.replica.on_ack(&key, seq) {
                    self.dropped(dropped);
                    self.changed(self.replica.actor().to_string(), false, Vec::new(), fx);
                }
            }
            Frame::Event {
                seq,
                actor,
                key,
                command,
            } => self.server_event(now_ms, seq, &actor, key.as_deref(), &command, fx),
        }
    }

    fn server_event(
        &mut self,
        now_ms: u64,
        seq: u64,
        actor: &str,
        key: Option<&str>,
        command: &Command,
        fx: &mut Vec<Effect>,
    ) {
        match self.replica.on_event_keyed(seq, actor, key, command) {
            Ok(EventOutcome::Confirmed) => {
                self.gap_closed();
                self.record_op(seq, actor, key, command, now_ms);
                self.stamp(actor, command);
            }
            Ok(EventOutcome::Rebased { dropped }) => {
                self.gap_closed();
                self.record_op(seq, actor, key, command, now_ms);
                self.stamp(actor, command);
                let nodes = if dropped > 0 {
                    Vec::new()
                } else {
                    render_set(std::slice::from_ref(command))
                };
                self.changed(actor.to_string(), false, nodes, fx);
            }
            Ok(EventOutcome::Duplicate) => self.level = Level::Nothing,
            Err(ReplicaError::Gap { .. }) => {
                self.level = Level::Nothing;
                self.on_gap(fx);
            }
            Err(ReplicaError::Diverged(_)) => {
                self.set_connection(Connection::Halted);
                self.events.push(Event::Diverged {});
            }
        }
    }

    pub(crate) fn dropped(&mut self, count: usize) {
        if count > 0 {
            self.events.push(Event::OpsDropped { count });
        }
    }

    /// Remember who changed the nodes a command touches.
    fn stamp(&mut self, actor: &str, command: &Command) {
        if !matches!(command, Command::RemoveNode { .. }) {
            for id in touched(command) {
                self.state.last_actor.insert(id, actor.to_string());
            }
        }
    }
}
