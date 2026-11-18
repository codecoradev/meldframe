//! MCP over streamable HTTP (`/mcp`), the third transport of the one gate. It is stateless: every request
//! carries its own Bearer token, and the principal of a tool call is the one that request authenticated as.
//!
//! Writes end in `Shared::submit_ops`, the handler REST and WebSocket end in (same authenticator, limits, slots
//! and submitter); reads go through the [`Reader`] seam on the same blocking pool. The tool text and schemas
//! are in [`TOOLS`]; `docs/mcp-tools.json` lists them and a test compares.

mod tools;
mod variant;

pub use meldframe_mcp_defs::{MAX_BATCH, MAX_KEY, TOOLS, ToolDef};

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
use std::{future::Future, pin::Pin, sync::Arc};

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
    /// The sequence number of the file, for a tool that reads it as part of its own call: not logged as a read
    /// call of its own. The default reads the whole file through [`Reader::view`].
    fn head(&self, who: &Principal, file: &str) -> Result<u64, Failure> {
        self.view(who, file).map(|v| v.seq)
    }
    /// Log a call that reads no file through this seam (`whoami`, a tool of another crate): `code` is `None` when
    /// it succeeded, else its stable error code. The default keeps no log.
    fn record(&self, _who: &Principal, _tool: &str, _file: &str, _code: Option<&str>) {}
}

/// A tool another crate contributes (capture): listed with the others, called with the request's principal.
pub trait Extra: std::fmt::Debug + Send + Sync {
    fn tool(&self) -> Tool;
    fn call(&self, who: Principal, args: serde_json::Map<String, Value>) -> ExtraCall<'_>;
}

pub type ExtraCall<'a> = Pin<Box<dyn Future<Output = CallToolResult> + Send + 'a>>;

/// Settings of the MCP route.
#[derive(Debug, Clone, Default)]
pub struct McpConfig {
    /// Tools of other crates to serve besides [`TOOLS`] (capture, upload_asset).
    pub extra: Vec<Arc<dyn Extra>>,
    /// Host names the route answers to besides the loopback names (`localhost`, `127.0.0.1`, `::1`), which it
    /// always answers to: the guard against DNS rebinding. A public deployment lists its own names.
    /// `Origin` is not validated: the route has no cookies and no CORS, a call needs the Bearer token.
    pub allowed_hosts: Vec<String>,
}

/// Registers the MCP tools in the registry with their tiers (`apply_commands` and `make_variant` are Changes, the rest Reads).
/// Without it `apply_commands` is refused as `forbidden`.
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
    extra: Vec<Arc<dyn Extra>>,
}

impl Handler {
    /// One entry in the call log through the reader, in a slot of the principal with the call time limit, like the
    /// other calls of the server (a busy server loses the entry rather than holding the reply).
    async fn record(&self, who: Principal, tool: String, file: String, code: Option<String>) {
        let (reader, w) = (self.reader.clone(), who.clone());
        let job = move || reader.record(&w, &tool, &file, code.as_deref());
        let _ = self.shared.blocking(&who, job).await;
    }
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
            "Meldframe design files. Call whoami first, read with get_tree and get_nodes, change with apply_commands (atomic, idempotent, ifSeq-guarded), copy a frame with make_variant and check a call with batch_status.",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(
            (TOOLS.iter().map(tool_of))
                .chain(self.extra.iter().map(|x| x.tool()))
                .collect(),
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
        if let Some(x) = self.extra.iter().find(|x| x.tool().name == request.name) {
            let file = a
                .get("file")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let out = x.call(who.clone(), a).await;
            let code = (out.is_error == Some(true)).then(|| {
                out.structured_content
                    .as_ref()
                    .and_then(|v| v["error"]["code"].as_str())
                    .unwrap_or("internal")
                    .to_owned()
            });
            self.record(who, request.name.to_string(), file, code).await;
            return Ok(out.into());
        }
        let body = match request.name.as_ref() {
            "whoami" => {
                let body = self.whoami(&who);
                self.record(who.clone(), "whoami".into(), String::new(), None)
                    .await;
                body
            }
            "get_tree" => self.get_tree(&who, a).await,
            "get_nodes" => self.get_nodes(&who, a).await,
            "apply_commands" => self.apply_commands(&who, a).await,
            "batch_status" => self.batch_status(&who, a).await,
            "make_variant" => self.make_variant(&who, a).await,
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
        extra: cfg.extra,
    };
    let service: StreamableHttpService<Handler, LocalSessionManager> =
        StreamableHttpService::new(move || Ok(handler.clone()), Default::default(), config);
    Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(shared, gate))
}
