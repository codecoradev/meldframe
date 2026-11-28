//! The limits of the adapter: [`Config`], the sizes, times, counts and budgets a call is held to.

use meldframe_ratelimit::{Clock, Rate};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Limits of the adapter; a request over one is refused with its own code.
#[derive(Clone)]
pub struct Config {
    /// Most bytes of a request body or a WebSocket frame.
    pub max_body: usize,
    /// Longest the caller waits for a call (the server's lock included); then `timeout`, and the job may still write.
    pub timeout: Duration,
    /// Longest the body of a request may take to arrive.
    pub body_timeout: Duration,
    /// Blocking calls running or abandoned at once, in all; one more is `unavailable`. A timed out call keeps its slot.
    pub max_in_flight: usize,
    /// The same, per principal.
    pub max_per_principal: usize,
    /// A WebSocket with no frame for this long is closed.
    pub ws_idle: Duration,
    /// A WebSocket is closed when its count of limited frames (up one per limited, down one per allowed) reaches this.
    pub ws_max_limited: u32,
    /// Longest a reply or the closing frame may take to send; a client that does not read is dropped.
    pub ws_send_timeout: Duration,
    /// Budget per peer address, spent before the token is checked.
    pub per_client: Rate,
    /// Budget per principal (workspace and actor), spent before the body is read.
    pub per_principal: Rate,
    pub clock: Clock,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("max_body", &self.max_body)
            .finish_non_exhaustive()
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            max_body: 1 << 20,
            timeout: Duration::from_secs(10),
            body_timeout: Duration::from_secs(10),
            max_in_flight: 32,
            max_per_principal: 4,
            ws_idle: Duration::from_secs(120),
            ws_max_limited: 5,
            ws_send_timeout: Duration::from_secs(10),
            per_client: Rate {
                burst: 60,
                per_second: 30,
            },
            per_principal: Rate {
                burst: 120,
                per_second: 60,
            },
            clock: {
                // Monotonic: a wall-clock step must not refill or freeze a bucket.
                let start = Instant::now();
                Arc::new(move || u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX))
            },
        }
    }
}
