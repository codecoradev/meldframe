//! The live connection: connecting, live, dropped, backoff, reconnect and resync. Time arrives as an
//! action payload (`now_ms`, the shell's clock); nothing here reads a clock.

use crate::app::{App, Level, out_ops};
use crate::effect::Effect;
use crate::event::Event;
use crate::retry::Retries;
use crate::state::Connection;
use meldframe_protocol::Node;

/// A live connection that has been silent this long is dropped. The server sends a heartbeat when it has
/// said nothing for 15 seconds, so three are missed by then.
pub const SILENT_MS: u64 = 45_000;
/// A first connection or a reconnect attempt that has not reached `Connected` after this long has failed.
pub const ATTEMPT_MS: u64 = 10_000;
/// A connection that stayed live this long is stable: the backoff starts again from the first step.
pub const STABLE_MS: u64 = 10_000;
const BACKOFF_FIRST_MS: u64 = 1_000;
const BACKOFF_MAX_MS: u64 = 30_000;

/// What the app remembers about the connection beyond the public state.
#[derive(Debug, Default)]
pub(crate) struct Link {
    /// Failed attempts since the connection was last stable.
    failures: u32,
    /// When the shell last heard from the server, or started the attempt that is under way.
    heard_ms: u64,
    /// When the connection became live, until it drops.
    live_since: Option<u64>,
    /// A subscribe for a gap is out; no second one until a frame is applied or the connection is new.
    resub: bool,
}

/// How long to wait before attempt number `step + 1`: 1 s, doubling, up to 30 s.
pub(crate) fn backoff(step: u32) -> u64 {
    let doubled = BACKOFF_FIRST_MS.saturating_mul(1u64 << step.saturating_sub(1).min(20));
    doubled.min(BACKOFF_MAX_MS)
}

impl App {
    /// Open the connection for the first time.
    pub(crate) fn start(&mut self, now_ms: u64, fx: &mut Vec<Effect>) {
        self.link.heard_ms = now_ms;
        self.link.resub = false;
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
        nodes: Vec<Node>,
        seq: u64,
        applied: &[String],
        fx: &mut Vec<Effect>,
    ) {
        match self.replica.resync(nodes, seq, applied) {
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
        self.prune_checkpoints();
        self.retries = Retries::default();
        self.link.live_since = None;
        self.start(now_ms, fx);
        self.changed(self.replica.actor().to_string(), false, Vec::new(), fx);
    }

    /// The transport is subscribed: the connection is live and everything unconfirmed goes out (again).
    /// The server dedups by key, so an op it already has is harmless.
    pub(crate) fn connected(&mut self, now_ms: u64, fx: &mut Vec<Effect>) {
        self.link.heard_ms = now_ms;
        self.link.live_since = Some(now_ms);
        self.link.resub = false;
        self.set_connection(Connection::Live);
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
        self.link.failures = self.link.failures.saturating_add(1);
        self.link.live_since = None;
        self.set_connection(Connection::Offline);
        fx.push(Effect::Reconnect {
            after_ms: backoff(self.link.failures),
        });
    }

    /// The backoff is over: try again from the last confirmed number.
    pub(crate) fn reconnect(&mut self, now_ms: u64, fx: &mut Vec<Effect>) {
        if self.state.connection != Connection::Offline {
            return;
        }
        self.link.heard_ms = now_ms;
        self.link.resub = false;
        self.set_connection(Connection::Reconnecting {
            attempt: self.link.failures,
        });
        fx.push(Effect::Subscribe {
            after_seq: self.replica.seq(),
        });
    }

    /// The shell's timer: a silent connection or a stuck attempt counts as dropped; due retries go out.
    pub(crate) fn tick(&mut self, now_ms: u64, fx: &mut Vec<Effect>) {
        let silent = now_ms.saturating_sub(self.link.heard_ms);
        let limit = match self.state.connection {
            Connection::Live => {
                self.stable(now_ms);
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
            self.link.heard_ms = now_ms;
            self.stable(now_ms);
        }
    }

    fn stable(&mut self, now_ms: u64) {
        if self
            .link
            .live_since
            .is_some_and(|since| now_ms.saturating_sub(since) >= STABLE_MS)
        {
            self.link.failures = 0;
        }
    }

    /// Frames after `after_seq` are lost: subscribe again from there.
    pub(crate) fn resubscribe(&mut self, after_seq: u64, fx: &mut Vec<Effect>) {
        self.link.resub = true;
        fx.push(Effect::Subscribe { after_seq });
    }

    /// A gap in the numbers: subscribe again from the last applied number, once until a frame applies.
    pub(crate) fn on_gap(&mut self, fx: &mut Vec<Effect>) {
        if !self.link.resub {
            self.resubscribe(self.replica.seq(), fx);
        }
    }

    /// A frame was applied: the subscribe for a gap did its work.
    pub(crate) fn gap_closed(&mut self) {
        self.link.resub = false;
    }
}
