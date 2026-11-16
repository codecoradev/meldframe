//! MCP over streamable HTTP (`/mcp`), the third transport of the one gate. It is stateless: every request
//! carries its own Bearer token, and the principal of a tool call is the one that request authenticated as.
//!
//! Writes end in `Shared::submit_ops`, the handler REST and WebSocket end in, so the authenticator, the rate
//! limits, the slots and the submitter are the same. Reads go through the [`Reader`] seam on the same blocking
//! pool. The tool text and schemas are in [`TOOLS`]; `docs/mcp-tools.json` lists them and a test compares.

mod defs;
mod tools;
mod view;

pub use defs::{MAX_BATCH, MAX_KEY, TOOLS, ToolDef};

use crate::{Failure, Shared, rest};
use axum::{
    Router,
    extract::{Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use meldframe_host::{Principal, StoredOp};
use meldframe_module::{Module, Registrar};
use meldframe_protocol::Node;
use rmcp::{
    ErrorData, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool, ToolAnnotations,
    },
    service::{RequestContext, RoleServer},
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde_json::Value;
use std::sync::Arc;

/// A file as a read tool sees it: its sequence number and its nodes.
#[derive(Debug, Clone)]
pub struct FileView {
    pub seq: u64,
    pub nodes: Vec<Node>,
}

/// What a conflict reply says: the file's sequence number, the node now (`None` when removed) and the
/// ops that touched it after `ifSeq`, oldest first.
#[derive(Debug, Clone)]
pub struct Conflict {
    pub seq: u64,
    pub node: Option<Node>,
    pub changes: Vec<StoredOp>,
}

/// Read access to files, for the read tools. The server implements it next to its `Submitter`; the
/// implementations block, so the adapter calls them from `spawn_blocking` under the call's slot. Each method
/// is one call under one lock on one opening of the file, so what it returns is consistent.
pub trait Reader: Send + Sync {
    /// The file of the principal's workspace as it is now (a file that does not exist reads as seq 0, empty).
    fn view(&self, who: &Principal, file: &str) -> Result<FileView, Failure>;
    /// The file and the last `window` ops of its log (fewer when the log is shorter), in order.
    fn tree(
        &self,
        who: &Principal,
        file: &str,
        window: usize,
    ) -> Result<(FileView, Vec<StoredOp>), Failure>;
    /// The detail of a conflict on `node`: at most the newest `limit` ops after `since` that touched it,
    /// out of the last [`meldframe_batch::MAX_SCAN`] ops of the log.
    fn conflict(
        &self,
        who: &Principal,
        file: &str,
        since: u64,
        node: &str,
        limit: usize,
    ) -> Result<Conflict, Failure>;
    /// For each key, the sequence number of the principal's op with that key, if it was written.
    fn applied(
        &self,
        who: &Principal,
        file: &str,
        keys: &[String],
    ) -> Result<Vec<Option<u64>>, Failure>;
}

/// Settings of the MCP route.
#[derive(Debug, Clone, Default)]
pub struct McpConfig {
    /// Host names the route answers to besides the loopback names (`localhost`, `127.0.0.1`, `::1`), which it
    /// always answers to: the guard against DNS rebinding. A public deployment lists its own names.
    /// `Origin` is not validated: the route has no cookies and no CORS, a call needs the Bearer token.
    pub allowed_hosts: Vec<String>,
}

/// Registers the MCP tools in the registry, with their tiers: the write tool is a Change, the others Reads.
/// The server compiles it in with its `mcp` feature; without it `apply_commands` is refused as `forbidden`.
#[derive(Debug, Clone, Copy)]
pub struct McpModule;

impl Module for McpModule {
    fn name(&self) -> &'static str {
        "mcp"
    }

    fn register(&self, r: &mut Registrar) {
        for t in &TOOLS {
            r.agent_tool(t.name, t.description, t.tier);
        }
    }
}

/// The per-request state of a tool call: the shared handler state and the reader.
#[derive(Clone)]
pub(crate) struct Handler {
    pub(crate) shared: Arc<Shared>,
    pub(crate) reader: Arc<dyn Reader>,
}

fn tool_of(d: &ToolDef) -> Tool {
    let schema = match d.input_schema() {
        Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    let read_only = d.tier == meldframe_module::RiskTier::Read;
    Tool::new(d.name, d.description, schema).annotate(
        ToolAnnotations::new()
            .read_only(read_only)
            .destructive(false)
            .idempotent(true),
    )
}

impl ServerHandler for Handler {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Meldframe design files. Call whoami first, read with get_tree and get_nodes, change with apply_commands (atomic, idempotent, ifSeq-guarded) and check a call with batch_status.",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(
            TOOLS.iter().map(tool_of).collect(),
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let who = context
            .extensions
            .get::<axum::http::request::Parts>()
            .and_then(|p| p.extensions.get::<Principal>())
            .cloned()
            .ok_or_else(|| ErrorData::internal_error("the call carries no principal", None))?;
        let a = request.arguments.unwrap_or_default();
        let body = match request.name.as_ref() {
            "whoami" => self.whoami(&who),
            "get_tree" => self.get_tree(&who, a).await,
            "get_nodes" => self.get_nodes(&who, a).await,
            "apply_commands" => self.apply_commands(&who, a).await,
            "batch_status" => self.batch_status(&who, a).await,
            other => {
                return Err(ErrorData::invalid_params(
                    format!("unknown tool {other}; list the tools with tools/list"),
                    None,
                ));
            }
        };
        Ok(match body {
            Ok(v) => CallToolResult::structured(v),
            Err(v) => CallToolResult::structured_error(v),
        }
        .into())
    }
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
    };
    let service: StreamableHttpService<Handler, LocalSessionManager> =
        StreamableHttpService::new(move || Ok(handler.clone()), Default::default(), config);
    Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(shared, gate))
}
