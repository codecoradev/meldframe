//! The live connection: connecting, live, dropped, backoff, reconnect and resync. Time arrives as an
//! action payload (`now_ms`, the shell's clock); nothing here reads a clock.

use crate::app::App;
use crate::effect::Effect;
use crate::event::Event;
use crate::state::Connection;
use meldframe_app_parts::document::out_ops;
use meldframe_app_rules::level::Level;
use meldframe_app_rules::retry::Retries;
use meldframe_protocol::Doc;

pub use meldframe_app_rules::link::{ATTEMPT_MS, SILENT_MS};

impl App {
    /// Open the connection for the first time.
    pub(crate) fn start(&mut self, now_ms: u64, fx: &mut Vec<Effect>) {
        self.link.attempt(now_ms);
        self.set_connection(Connection::Connecting);
        fx.push(Effect::Subscribe {
            after_seq: self.replica.seq(),
        });
    }

    /// The shell loaded the file again: the replica starts from the snapshot and the app subscribes from
    /// its number. A bad snapshot changes nothing.
    pub(crate) fn reload(
        &mut self,
        now_ms: u64,
        doc: Doc,
        seq: u64,
        applied: &[String],
        fx: &mut Vec<Effect>,
    ) {
        match self.replica.resync_doc(doc, seq, applied) {
            Ok(dropped) => self.dropped(dropped),
            Err(e) => {
                self.level = Level::Nothing;
                self.events.push(Event::Refused {
                    code: e.code().to_string(),
                });
                return;
            }
        }
        self.review
            .reset(self.replica.base().to_vec(), self.replica.seq());
        self.feed.reset(self.replica.seq());
        self.checkpoints_reset();
        self.retries = Retries::default();
        self.link.reloaded();
        self.start(now_ms, fx);
        self.changed(self.replica.actor().to_string(), false, Vec::new(), fx);
    }

    /// The transport is subscribed: the connection is live and everything unconfirmed goes out (again).
    /// The server dedups by key, so an op it already has is harmless.
    pub(crate) fn connected(&mut self, now_ms: u64, fx: &mut Vec<Effect>) {
        self.link.live(now_ms);
        self.set_connection(Connection::Live);
        self.fetched(|f, _| f.connected(now_ms), fx);
        self.checkpoints_connected(fx);
        let ops = out_ops(self.replica.unconfirmed());
        // What the replica queued before is part of `unconfirmed`: do not send it twice.
        self.replica.take_outbox();
        if !ops.is_empty() {
            fx.push(Effect::SendOps { ops });
        }
    }

    /// The connection dropped, or an attempt failed: go offline and ask for a retry after the backoff.
    pub(crate) fn disconnected(&mut self, fx: &mut Vec<Effect>) {
        if matches!(
            self.state.connection,
            Connection::Offline | Connection::Halted
        ) {
            return;
        }
        let after_ms = self.link.failed();
        self.set_connection(Connection::Offline);
        fx.push(Effect::Reconnect { after_ms });
    }

    /// The backoff is over: try again from the last confirmed number.
    pub(crate) fn reconnect(&mut self, now_ms: u64, fx: &mut Vec<Effect>) {
        if self.state.connection != Connection::Offline {
            return;
        }
        self.link.attempt(now_ms);
        self.set_connection(Connection::Reconnecting {
            attempt: self.link.failures(),
        });
        fx.push(Effect::Subscribe {
            after_seq: self.replica.seq(),
        });
    }

    /// The shell's timer: a silent connection or a stuck attempt counts as dropped; due retries go out.
    pub(crate) fn tick(&mut self, now_ms: u64, fx: &mut Vec<Effect>) {
        self.fetched(|f, live| f.tick(now_ms, live), fx);
        let silent = self.link.silent_for(now_ms);
        let limit = match self.state.connection {
            Connection::Live => {
                self.link.stable(now_ms);
                SILENT_MS
            }
            Connection::Connecting | Connection::Reconnecting { .. } => ATTEMPT_MS,
            _ => return,
        };
        if silent > limit {
            self.disconnected(fx);
        } else {
            self.send_due_retries(now_ms, fx);
        }
    }

    /// Something arrived from the server.
    pub(crate) fn heard(&mut self, now_ms: u64) {
        if self.state.connection == Connection::Live {
            self.link.heard(now_ms);
        }
    }

    /// Frames after `after_seq` are lost: subscribe again from there.
    pub(crate) fn resubscribe(&mut self, after_seq: u64, fx: &mut Vec<Effect>) {
        self.link.resubscribing();
        fx.push(Effect::Subscribe { after_seq });
    }

    /// A gap in the numbers: subscribe again from the last applied number, once until a frame applies.
    pub(crate) fn on_gap(&mut self, fx: &mut Vec<Effect>) {
        if !self.link.is_resubscribing() {
            self.resubscribe(self.replica.seq(), fx);
        }
    }

    /// A frame was applied: the subscribe for a gap did its work.
    pub(crate) fn gap_closed(&mut self) {
        self.link.gap_closed();
    }
}
