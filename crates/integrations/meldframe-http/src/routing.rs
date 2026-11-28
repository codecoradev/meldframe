//! The router: REST and WebSocket over one submitter, one authenticator and one set of limits, and MCP when
//! its feature is on.

use crate::auth::Authenticator;
use crate::config::Config;
use crate::shared::{Shared, shared};
use crate::submitter::Submitter;
use crate::{rest, ws};
use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;

/// REST and `/v1/ws` over `shared`.
pub fn routes(shared: Arc<Shared>) -> Router {
    Router::new()
        .route("/v1/files/{file}/tools/{tool}", post(rest::submit))
        .route("/v1/ws", get(ws::upgrade))
        .with_state(shared)
}

/// The router: REST and WebSocket over one submitter, one authenticator and one set of limits.
pub fn router(
    auth: Arc<dyn Authenticator>,
    submitter: Arc<dyn Submitter>,
    config: Config,
) -> Router {
    routes(shared(auth, submitter, config))
}

/// The router with MCP (`/mcp`) added: all three transports share one authenticator, limits and submitter.
#[cfg(feature = "mcp")]
pub fn router_with_mcp_config(
    auth: Arc<dyn Authenticator>,
    submitter: Arc<dyn Submitter>,
    reader: Arc<dyn crate::mcp::Reader>,
    config: Config,
    mcp: crate::mcp::McpConfig,
) -> Router {
    let shared = shared(auth, submitter, config);
    routes(shared.clone()).merge(crate::mcp::routes(shared, reader, mcp))
}
