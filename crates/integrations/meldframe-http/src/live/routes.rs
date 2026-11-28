use axum::{Router, routing::get};
use meldframe_http::{Authenticator, Clock, Config, Shared, Slots, Submitter};
use std::sync::Arc;
use std::time::Duration;

use super::hub::{LiveHub, LiveSource, Timer};
use super::{LiveConfig, ws};

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
    let hub = Arc::new(LiveHub::new(source, clock, poll, config.hook.clone()));
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
