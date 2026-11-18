//! A token bucket per key, with a clock the caller supplies so tests can move time by hand.

use std::collections::{BTreeMap, HashMap};
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

/// Most buckets kept at once; past it, the key checked longest ago is forgotten (never the whole table).
const MAX_KEYS: usize = 10_000;

#[derive(Clone, Copy)]
struct Bucket {
    /// Thousandths of a token.
    level: u64,
    at: u64,
    /// Order of the last check of this key, for eviction.
    used: u64,
}

#[derive(Default)]
struct Table {
    map: HashMap<String, Bucket>,
    /// Keys by last use, oldest first.
    order: BTreeMap<u64, String>,
    tick: u64,
}

/// Limits calls per key. A call costs one token.
pub struct RateLimiter {
    rate: Rate,
    clock: Clock,
    capacity: usize,
    table: Mutex<Table>,
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
        Self::with_capacity(rate, clock, MAX_KEYS)
    }

    /// Like [`RateLimiter::new`], keeping at most `capacity` keys.
    pub fn with_capacity(rate: Rate, clock: Clock, capacity: usize) -> Self {
        RateLimiter {
            rate,
            clock,
            capacity: capacity.max(1),
            table: Mutex::default(),
        }
    }

    /// Spend one token of `key`; `Err` says how long until one is there.
    pub fn check(&self, key: &str) -> Result<(), Duration> {
        let now = (self.clock)();
        let cap = u64::from(self.rate.burst) * 1000;
        let mut t = self.table.lock().unwrap_or_else(|p| p.into_inner());
        t.tick += 1;
        let used = t.tick;
        if t.map.len() >= self.capacity
            && !t.map.contains_key(key)
            && let Some((_, oldest)) = t.order.pop_first()
        {
            t.map.remove(&oldest);
        }
        let before = t.map.get(key).copied();
        if let Some(b) = before {
            t.order.remove(&b.used);
        }
        t.order.insert(used, key.to_owned());
        let b = before.unwrap_or(Bucket {
            level: cap,
            at: now,
            used,
        });
        let level = self.refilled(b, now, cap);
        let spent = level >= 1000;
        let level = if spent { level - 1000 } else { level };
        t.map.insert(
            key.to_owned(),
            Bucket {
                level,
                at: now,
                used,
            },
        );
        if spent {
            return Ok(());
        }
        let per_second = u64::from(self.rate.per_second.max(1));
        Err(Duration::from_millis((1000 - level).div_ceil(per_second)))
    }

    fn refilled(&self, b: Bucket, now: u64, cap: u64) -> u64 {
        let added = now
            .saturating_sub(b.at)
            .saturating_mul(u64::from(self.rate.per_second));
        b.level.saturating_add(added).min(cap)
    }

    /// Whether the eviction index and the table hold the same keys with the same last-use marks.
    #[doc(hidden)]
    pub fn consistent(&self) -> bool {
        let t = self.table.lock().unwrap_or_else(|p| p.into_inner());
        t.map.len() == t.order.len()
            && t.order
                .iter()
                .all(|(used, key)| t.map.get(key).is_some_and(|b| b.used == *used))
    }

    /// How many buckets are kept.
    pub fn len(&self) -> usize {
        self.table
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .map
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
