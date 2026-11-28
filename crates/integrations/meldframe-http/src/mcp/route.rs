//! The `/mcp` route: the settings it takes, the authentication in front of it and the service behind it.

use super::{Extra, Handler, Reader};
use crate::{rest, shared::Shared};
use axum::{
    Router,
    extract::{Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use std::sync::Arc;

/// Settings of the MCP route.
#[derive(Debug, Clone, Default)]
pub struct McpConfig {
    /// Tools of other crates to serve besides [`super::TOOLS`] (capture, upload_asset, make_variant).
    pub extra: Vec<Arc<dyn Extra>>,
    /// Host names the route answers to besides the loopback names (`localhost`, `127.0.0.1`, `::1`), which it
    /// always answers to: the guard against DNS rebinding. A public deployment lists its own names.
    /// `Origin` is not validated: the route has no cookies and no CORS, a call needs the Bearer token.
    pub allowed_hosts: Vec<String>,
}

/// Authenticate before MCP sees the request, with the same checks and budgets as REST: the client's rate, the
/// token, the principal's rate. The principal goes into the request for the tool call.
async fn gate(State(s): State<Arc<Shared>>, mut req: Request, next: Next) -> Response {
    let who = s
        .client_ok(&rest::peer(&req))
        .and_then(|()| s.principal(rest::bearer(req.headers())));
    match who {
        Ok(who) => {
            req.extensions_mut().insert(who);
            next.run(req).await
        }
        Err(reply) => reply.into_response(),
    }
}

/// The `/mcp` route over `shared`, to merge into the router of the same `shared`.
pub fn routes(shared: Arc<Shared>, reader: Arc<dyn Reader>, cfg: McpConfig) -> Router {
    let mut config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_max_request_body_bytes(shared.max_body);
    let loopback = ["localhost", "127.0.0.1", "::1"].map(String::from);
    config = config.with_allowed_hosts(loopback.into_iter().chain(cfg.allowed_hosts));
    let handler = Handler {
        shared: shared.clone(),
        reader,
        extra: cfg.extra,
    };
    let service: StreamableHttpService<Handler, LocalSessionManager> =
        StreamableHttpService::new(move || Ok(handler.clone()), Default::default(), config);
    Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(shared, gate))
}
