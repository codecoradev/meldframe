//! The live connection: connecting, live, dropped, backoff, reconnect and resync. Time arrives as an
//! action payload (`now_ms`, the shell's clock); nothing here reads a clock.

use crate::app::{App, out_ops};
use crate::effect::Effect;
use crate::state::Connection;

/// A live connection that has been silent this long is dropped. The server sends a heartbeat when it has
/// said nothing for 15 seconds, so three are missed by then.
pub const SILENT_MS: u64 = 45_000;
/// A reconnect attempt that has not reached `Connected` after this long has failed.
pub const ATTEMPT_MS: u64 = 10_000;
const BACKOFF_FIRST_MS: u64 = 1_000;
const BACKOFF_MAX_MS: u64 = 30_000;

/// What the app remembers about the connection beyond the public state.
#[derive(Debug, Default)]
pub(crate) struct Link {
    /// Failed attempts since the last `Connected`.
    failures: u32,
    /// When the shell last heard from the server, or started the attempt that is under way.
    heard_ms: u64,
}

/// How long to wait before attempt number `failures + 1`: 1 s, doubling, up to 30 s.
fn backoff(failures: u32) -> u64 {
    let doubled = BACKOFF_FIRST_MS.saturating_mul(1u64 << failures.saturating_sub(1).min(20));
    doubled.min(BACKOFF_MAX_MS)
}

impl App {
    /// Open the connection for the first time.
    pub(crate) fn start(&mut self, fx: &mut Vec<Effect>) {
        self.set_connection(Connection::Connecting);
        fx.push(Effect::Subscribe {
            after_seq: self.replica.seq(),
        });
    }

    /// The transport is subscribed: the connection is live and everything unconfirmed goes out (again).
    /// The server dedups by key, so an op it already has is harmless.
    pub(crate) fn connected(&mut self, now_ms: u64, fx: &mut Vec<Effect>) {
        self.link.failures = 0;
        self.link.heard_ms = now_ms;
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
        if self.state.connection == Connection::Offline {
            return;
        }
        self.link.failures = self.link.failures.saturating_add(1);
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
        self.set_connection(Connection::Reconnecting {
            attempt: self.link.failures,
        });
        fx.push(Effect::Subscribe {
            after_seq: self.replica.seq(),
        });
    }

    /// The shell's timer: a silent connection or a stuck attempt counts as dropped.
    pub(crate) fn tick(&mut self, now_ms: u64, fx: &mut Vec<Effect>) {
        let silent = now_ms.saturating_sub(self.link.heard_ms);
        let limit = match self.state.connection {
            Connection::Live => SILENT_MS,
            Connection::Reconnecting { .. } => ATTEMPT_MS,
            _ => return,
        };
        if silent > limit {
            self.disconnected(fx);
        }
    }

    /// Something arrived from the server.
    pub(crate) fn heard(&mut self, now_ms: u64) {
        if self.state.connection == Connection::Live {
            self.link.heard_ms = now_ms;
        }
    }

    /// Frames after `after_seq` are lost (a resync marker, or a gap in the numbers): subscribe again from there.
    pub(crate) fn resubscribe(after_seq: u64, fx: &mut Vec<Effect>) {
        fx.push(Effect::Subscribe { after_seq });
    }
}
