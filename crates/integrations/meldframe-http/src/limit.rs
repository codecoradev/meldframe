//! A token bucket per key, with a clock the caller supplies so tests can move time by hand.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Milliseconds since any fixed point; only differences matter.
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// How fast a bucket refills and how much it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    pub burst: u32,
    pub per_second: u32,
}

/// Most buckets kept at once; past it, buckets that are full again (and so equal a fresh one) go first.
const MAX_KEYS: usize = 10_000;

#[derive(Clone, Copy)]
struct Bucket {
    /// Thousandths of a token.
    level: u64,
    at: u64,
}

/// Limits calls per key. A call costs one token.
pub struct RateLimiter {
    rate: Rate,
    clock: Clock,
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter")
            .field("rate", &self.rate)
            .finish_non_exhaustive()
    }
}

impl RateLimiter {
    pub fn new(rate: Rate, clock: Clock) -> Self {
        RateLimiter {
            rate,
            clock,
            buckets: Mutex::default(),
        }
    }

    /// Spend one token of `key`; `Err` says how long until one is there.
    pub fn check(&self, key: &str) -> Result<(), Duration> {
        let now = (self.clock)();
        let cap = u64::from(self.rate.burst) * 1000;
        let mut map = self.buckets.lock().unwrap_or_else(|p| p.into_inner());
        if map.len() >= MAX_KEYS && !map.contains_key(key) {
            map.retain(|_, b| self.refilled(*b, now, cap) < cap);
            if map.len() >= MAX_KEYS {
                map.clear();
            }
        }
        let b = map.entry(key.to_owned()).or_insert(Bucket {
            level: cap,
            at: now,
        });
        let level = self.refilled(*b, now, cap);
        if level >= 1000 {
            *b = Bucket {
                level: level - 1000,
                at: now,
            };
            return Ok(());
        }
        *b = Bucket { level, at: now };
        let per_second = u64::from(self.rate.per_second.max(1));
        Err(Duration::from_millis((1000 - level).div_ceil(per_second)))
    }

    fn refilled(&self, b: Bucket, now: u64, cap: u64) -> u64 {
        let added = now
            .saturating_sub(b.at)
            .saturating_mul(u64::from(self.rate.per_second));
        b.level.saturating_add(added).min(cap)
    }

    /// How many buckets are kept.
    pub fn len(&self) -> usize {
        self.buckets.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
