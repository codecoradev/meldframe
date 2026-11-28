//! The one call the binary makes to wire checkpoints, and its variant for a sink built by the caller.

use crate::gate::Gate;
use crate::routes;
use crate::sink::Checkpoints;
use checkpoint_store::CheckpointStore;
use meldframe_http::Shared;
use meldframe_http::mcp::Reader;
use std::sync::Arc;

/// Everything the server wires for checkpoints in one call: the sink is set on `shared`, so every write of an agent is
/// watched and a run start is announced, and the routes come back to merge into the router. A second install on the
/// same `Shared` keeps the first sink. The routes of the list, the create and the state are in the router.
pub fn install(
    shared: &Arc<Shared>,
    store: Arc<dyn CheckpointStore>,
    reader: Arc<dyn Reader>,
    gate: Gate,
) -> axum::Router {
    install_with(shared, Checkpoints::new(store, reader), gate).1
}

/// As [`install`], for a sink built by the caller (a test gives it a clock); the sink comes back too, for its failure
/// count.
pub fn install_with(
    shared: &Arc<Shared>,
    mut checkpoints: Checkpoints,
    gate: Gate,
) -> (Arc<Checkpoints>, axum::Router) {
    checkpoints.live = Some(gate.publish.clone());
    let checkpoints = Arc::new(checkpoints);
    let _ = shared.runs.set(checkpoints.clone());
    let router = routes::routes(shared.clone(), checkpoints.clone(), gate);
    (checkpoints, router)
}
