use std::sync::Arc;
use std::time::Duration;

use super::hub::ClientHook;

/// Limits of the live transport. The handshake is an ordinary call and uses the adapter's `Config`; the
/// settings here are for what a connection does after it.
#[derive(Debug, Clone)]
pub struct LiveConfig {
    /// How often the timer runs `Server::tick` (heartbeats, reaping); see [`Timer`].
    pub live_tick: Duration,
    /// How often a live connection looks for frames even when nothing woke it (ephemeral frames and
    /// heartbeats are not announced by an accepted op). At least 10 ms, and at most the server's heartbeat
    /// interval when the source says it (a longer value is cut to it).
    pub live_poll: Duration,
    /// Live calls after the handshake (poll, resubscribe, unsubscribe) running or abandoned at once, in all.
    /// They have their own slots, apart from the adapter's `max_in_flight`, so sockets cannot starve REST. A
    /// call with no slot is skipped and tried again at the next look.
    pub max_live_in_flight: usize,
    /// The same, per principal.
    pub max_live_per_principal: usize,
    /// `seen` calls (one per client frame) running or abandoned at once, in all. They have places of their
    /// own, apart from the live calls, so a client that pings stays alive while the live places are taken.
    pub max_seen_in_flight: usize,
    /// The same, per principal. A `seen` with no place is done at the connection's next look.
    pub max_seen_per_principal: usize,
    /// A feature's hook on the sockets (see [`ClientHook`]); none by default.
    pub hook: Option<Arc<dyn ClientHook>>,
}

impl Default for LiveConfig {
    fn default() -> Self {
        LiveConfig {
            live_tick: Duration::from_secs(1),
            live_poll: Duration::from_millis(250),
            max_live_in_flight: 16,
            max_live_per_principal: 8,
            max_seen_in_flight: 8,
            max_seen_per_principal: 2,
            hook: None,
        }
    }
}
