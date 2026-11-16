//! The live WebSocket of the HTTP adapter (ADR-0009, E1.4): `GET /v1/live/{file}?after={seq}`. It is its own
//! socket, not a message on `/v1/ws`: the handshake can answer 401, 429 or 503 as a status, one connection is
//! one stream with one lifetime (so its heartbeat, reaping and ownership need no routing table of requests),
//! and the submit socket stays plain request and reply. The protocol is in [`ws`](crate) (`ws.rs`).
//!
//! It builds on `meldframe-http`: the same authenticator, per-address and per-principal budgets and size
//! limit through [`Shared`], and a hook on [`Shared::on_write`] that wakes the sockets of a file when an op of
//! it was accepted over REST, `/v1/ws` or MCP. [`routes`] adds the route to any router built over the same
//! `Shared`, and returns the [`Timer`] to spawn; [`live_router`] does both steps for the REST and `/v1/ws`
//! routes. Without the timer no heartbeat is sent and a vanished client keeps its stream.

mod hub;
mod ws;

use axum::{Router, routing::get};
use hub::LiveHub;
pub use hub::{LiveSource, Opened, Out, Timer};
use meldframe_http::{Authenticator, Clock, Config, Shared, Slots, Submitter};
use std::sync::Arc;
use std::time::Duration;

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
        }
    }
}

/// State of the live route.
pub(crate) struct LiveState {
    pub(crate) shared: Arc<Shared>,
    pub(crate) hub: Arc<LiveHub>,
    pub(crate) live_slots: Slots,
    pub(crate) seen_slots: Slots,
}

/// The live route over `shared`, to merge into the router of the same `shared`, and the [`Timer`] to spawn on
/// the runtime that serves it. `clock` is the adapter's clock (`Config::clock`), the one every live call and
/// the timer use. Sets [`Shared::on_write`], which a `Shared` takes once.
pub fn routes(
    shared: Arc<Shared>,
    source: Arc<dyn LiveSource>,
    config: LiveConfig,
    clock: Clock,
) -> (Router, Timer) {
    // At least 10 ms (a tighter loop only spins the blocking pool), and at most the server's heartbeat
    // interval, so a heartbeat is sent at most one look after it is due.
    let mut poll = config.live_poll.max(Duration::from_millis(10));
    if let Some(beat) = source.heartbeat_ms() {
        poll = poll.min(Duration::from_millis(beat).max(Duration::from_millis(10)));
    }
    let hub = Arc::new(LiveHub::new(source, clock, poll));
    let woken = hub.clone();
    let _ = shared
        .on_write
        .set(Box::new(move |who, file| woken.wake(who, file)));
    let state = Arc::new(LiveState {
        shared,
        hub: hub.clone(),
        live_slots: Slots::new(config.max_live_in_flight, config.max_live_per_principal),
        seen_slots: Slots::new(config.max_seen_in_flight, config.max_seen_per_principal),
    });
    let router = Router::new()
        .route("/v1/live/{file}", get(ws::upgrade))
        .with_state(state);
    (
        router,
        Timer {
            hub,
            every: config.live_tick,
        },
    )
}

/// REST and `/v1/ws` of `meldframe-http` plus the live route, over one submitter, one authenticator and one
/// set of limits. Spawn the returned [`Timer`] on the runtime that serves the router.
pub fn live_router(
    auth: Arc<dyn Authenticator>,
    submitter: Arc<dyn Submitter>,
    source: Arc<dyn LiveSource>,
    config: Config,
    live: LiveConfig,
) -> (Router, Timer) {
    let clock = config.clock.clone();
    let shared = meldframe_http::shared(auth, submitter, config);
    let (route, timer) = routes(shared.clone(), source, live, clock);
    (meldframe_http::routes(shared).merge(route), timer)
}
