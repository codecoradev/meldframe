//! The frames the server sends: heartbeat, resync, batch and touch, acks, refusals and live events.

use crate::action::Frame;
use crate::app::App;
use crate::effect::Effect;
use crate::event::Event;
use crate::parts::touched::{render_set, touched};
use crate::state::Connection;
use crate::text;
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
            Frame::SubscribeRefused {
                code,
                retry_after_ms,
            } => self.subscribe_refused(now_ms, code.as_deref(), retry_after_ms, fx),
            Frame::Checkpoint { checkpoint } => self.checkpoint_frame(checkpoint, fx),
            Frame::Batch { .. } => self.activity_frame(now_ms, frame, fx),
            // The actor of a touch is a lane key, so it is made safe here; an actor with nothing left draws nothing.
            Frame::Touch { actor, ids, seq } => {
                if let Some(actor) = text::actor(&actor) {
                    self.activity_frame(now_ms, Frame::Touch { actor, ids, seq }, fx);
                }
            }
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
        // The replica takes the op as the server sent it; everything the app keys and shows by gets the actor and the
        // key made safe once, here (`crate::text`).
        let who = text::actor(actor);
        let shown = who.as_deref().unwrap_or_default();
        match self.replica.on_event_keyed(seq, actor, key, command) {
            Ok(EventOutcome::Confirmed) => {
                self.gap_closed();
                self.record_op(seq, shown, key, command, now_ms);
                self.stamp(who.as_deref(), command);
            }
            Ok(EventOutcome::Rebased { dropped }) => {
                self.gap_closed();
                self.record_op(seq, shown, key, command, now_ms);
                self.stamp(who.as_deref(), command);
                let nodes = if dropped > 0 {
                    Vec::new()
                } else {
                    render_set(std::slice::from_ref(command))
                };
                self.changed(shown.to_string(), false, nodes, fx);
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

    /// Remember who changed the nodes a command touches. An op by an actor with no name is attributed to nobody.
    fn stamp(&mut self, actor: Option<&str>, command: &Command) {
        let Some(actor) = actor else { return };
        if !matches!(command, Command::RemoveNode { .. }) {
            for id in touched(command) {
                self.state.last_actor.insert(id, actor.to_string());
            }
        }
    }
}
