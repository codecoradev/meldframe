//! Retries of an op the server refused as retryable (a quota, a dependency still pending): the tries made and when each
//! waiting op is due. The app keeps the op and sends it again with the same key; this holds the count and the clock.

use crate::link::backoff;
use std::collections::BTreeMap;

/// How many times an op is sent again after a retryable refusal.
pub const MAX_RETRIES: u32 = 3;

/// Retries under way: tries made so far per op key, and when each waiting op is due.
#[derive(Debug, Default)]
pub struct Retries {
    tries: BTreeMap<String, u32>,
    due: Vec<(u64, String)>,
}

/// What to do with a refused op.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Send it again after `after_ms` (this is try number `attempt`).
    Retry { attempt: u32, after_ms: u64 },
    /// Retryable, but not live: `Connected` sends every unconfirmed op again anyway.
    Later,
    /// Give up: roll the op back. Its retry state is forgotten.
    Reject,
}

impl Retries {
    /// The server refused op `key` (`live`: the connection is live).
    pub fn on_refused(&mut self, key: &str, now_ms: u64, retryable: bool, live: bool) -> Refusal {
        let tries = self.tries.get(key).copied().unwrap_or(0);
        if retryable && tries < MAX_RETRIES {
            if !live {
                return Refusal::Later;
            }
            let attempt = tries + 1;
            let after_ms = backoff(attempt);
            self.tries.insert(key.to_string(), attempt);
            self.due
                .push((now_ms.saturating_add(after_ms), key.to_string()));
            return Refusal::Retry { attempt, after_ms };
        }
        self.tries.remove(key);
        self.due.retain(|(_, k)| k != key);
        Refusal::Reject
    }

    /// Whether any op waits for its retry.
    pub fn has_due(&self) -> bool {
        !self.due.is_empty()
    }

    /// The keys of the ops whose retry is due; the others keep waiting.
    pub fn take_due(&mut self, now_ms: u64) -> Vec<String> {
        let (due, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut self.due)
            .into_iter()
            .partition(|(at, _)| *at <= now_ms);
        self.due = wait;
        due.into_iter().map(|(_, k)| k).collect()
    }

    /// Forget the retry state of ops that are no longer pending.
    pub fn prune(&mut self, pending: &[String]) {
        self.tries.retain(|k, _| pending.contains(k));
        self.due.retain(|(_, k)| pending.contains(k));
    }
}
