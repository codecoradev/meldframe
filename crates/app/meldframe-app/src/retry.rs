//! A refusal the server calls retryable (a quota, a dependency still pending): keep the op and send it
//! again with the same key after a backoff, a few times, then give up and roll it back.

use crate::app::{App, out_ops};
use crate::effect::Effect;
use crate::event::Event;
use crate::link::backoff;
use crate::state::Connection;
use std::collections::BTreeMap;

/// How many times an op is sent again after a retryable refusal.
pub const MAX_RETRIES: u32 = 3;

/// Retries under way: tries made so far per op key, and when each waiting op is due.
#[derive(Debug, Default)]
pub(crate) struct Retries {
    tries: BTreeMap<String, u32>,
    due: Vec<(u64, String)>,
}

impl App {
    /// The server refused op `key`. Returns true when the op was rolled back (the view changed).
    pub(crate) fn refused(
        &mut self,
        now_ms: u64,
        key: String,
        code: String,
        retryable: bool,
        fx: &mut Vec<Effect>,
    ) -> bool {
        if !self.state.pending.contains(&key) {
            return false;
        }
        let tries = self.retries.tries.get(&key).copied().unwrap_or(0);
        if retryable && tries < MAX_RETRIES {
            if self.state.connection == Connection::Live {
                let attempt = tries + 1;
                let after_ms = backoff(attempt);
                self.retries.tries.insert(key.clone(), attempt);
                self.retries
                    .due
                    .push((now_ms.saturating_add(after_ms), key.clone()));
                self.events.push(Event::OpRetry { key, attempt });
                fx.push(Effect::WakeAfter { after_ms });
            }
            // Not live: `Connected` sends every unconfirmed op again anyway.
            return false;
        }
        self.retries.tries.remove(&key);
        self.retries.due.retain(|(_, k)| *k != key);
        if !self.replica.on_reject(&key) {
            return false;
        }
        self.events.push(Event::OpRejected {
            key,
            code,
            retryable,
        });
        true
    }

    /// Send the ops whose retry is due.
    pub(crate) fn send_due_retries(&mut self, now_ms: u64, fx: &mut Vec<Effect>) {
        if self.retries.due.is_empty() || self.state.connection != Connection::Live {
            return;
        }
        let (due, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut self.retries.due)
            .into_iter()
            .partition(|(at, _)| *at <= now_ms);
        self.retries.due = wait;
        if due.is_empty() {
            return;
        }
        let ops: Vec<_> = self
            .replica
            .unconfirmed()
            .into_iter()
            .filter(|o| due.iter().any(|(_, k)| *k == o.key))
            .collect();
        if !ops.is_empty() {
            fx.push(Effect::SendOps { ops: out_ops(ops) });
        }
    }

    /// Forget the retry state of ops that are no longer pending.
    pub(crate) fn prune_retries(&mut self) {
        let pending = &self.state.pending;
        self.retries.tries.retain(|k, _| pending.contains(k));
        self.retries.due.retain(|(_, k)| pending.contains(k));
    }
}
