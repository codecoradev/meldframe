//! Shared helpers: the real server behind the MCP router on a real socket, a spy that counts what reaches
//! the submitter and the reader, and an rmcp client.
#![allow(dead_code, reason = "each test binary uses a part of these helpers")]

use meldframe_host::{Accepted, Op, Principal, StoredOp};
use meldframe_http::mcp::{Conflict, FileView, Reader};
use meldframe_http::{Config, DevTokens, Failure, Guard, Rate, Submitter, router_with_mcp_config};
use meldframe_server::{Server, SharedServer};
use rmcp::{
    ClientServiceExt,
    model::{CallToolRequestParams, ClientConfig},
    service::{ClientLifecycleMode, RoleClient, RunningService},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const AGENT: &str = "agent-token-alpha-1";
pub const PERSON: &str = "person-token-beta-02";
pub const OTHER: &str = "other-token-gamma-03";

/// Counts what reaches the one submitter and the one reader, and forwards to the real server.
pub struct Spy {
    pub server: Arc<SharedServer>,
    pub submits: AtomicUsize,
    pub atomic_submits: AtomicUsize,
    pub reads: AtomicUsize,
}

impl Submitter for Spy {
    fn submit(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
    ) -> Result<Vec<Accepted>, Failure> {
        self.submits.fetch_add(1, Ordering::SeqCst);
        self.server.submit(tool, file, who, ops)
    }
    fn submit_atomic(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
        guards: Vec<Guard>,
    ) -> Result<Vec<Accepted>, Failure> {
        self.atomic_submits.fetch_add(1, Ordering::SeqCst);
        self.server.submit_atomic(tool, file, who, ops, guards)
    }
}

impl Reader for Spy {
    fn view(&self, who: &Principal, file: &str) -> Result<FileView, Failure> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.server.view(who, file)
    }
    fn tree(
        &self,
        who: &Principal,
        file: &str,
        window: usize,
    ) -> Result<(FileView, Vec<StoredOp>), Failure> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.server.tree(who, file, window)
    }
    fn conflict(
        &self,
        who: &Principal,
        file: &str,
        since: u64,
        node: &str,
        limit: usize,
    ) -> Result<Conflict, Failure> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.server.conflict(who, file, since, node, limit)
    }
    fn applied(
        &self,
        who: &Principal,
        file: &str,
        keys: &[String],
    ) -> Result<Vec<Option<u64>>, Failure> {
        self.server.applied(who, file, keys)
    }
}

pub struct Harness {
    pub addr: SocketAddr,
    pub spy: Arc<Spy>,
}

pub fn config() -> Config {
    Config {
        per_client: Rate {
            burst: 10_000,
            per_second: 10_000,
        },
        per_principal: Rate {
            burst: 10_000,
            per_second: 10_000,
        },
        ..Config::default()
    }
}

pub async fn start() -> Harness {
    start_with(config()).await
}

pub async fn start_with(config: Config) -> Harness {
    start_hosts(config, Vec::new()).await
}

/// Like `start_with`, with the extra `Host` names the MCP route answers to.
pub async fn start_hosts(config: Config, allowed_hosts: Vec<String>) -> Harness {
    let spy = Arc::new(Spy {
        server: Arc::new(SharedServer::new(Server::assemble())),
        submits: AtomicUsize::new(0),
        atomic_submits: AtomicUsize::new(0),
        reads: AtomicUsize::new(0),
    });
    let tokens = DevTokens::parse(&format!(
        "{AGENT}:ws_1:agent:alpha,{PERSON}:ws_1:user:bea,{OTHER}:ws_2:agent:gamma"
    ))
    .unwrap();
    let mcp = meldframe_http::mcp::McpConfig { allowed_hosts };
    let app = router_with_mcp_config(Arc::new(tokens), spy.clone(), spy.clone(), config, mcp);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    Harness { addr, spy }
}

pub type Client = RunningService<RoleClient, ClientConfig>;

/// A real MCP client over streamable HTTP, sending `token` as a Bearer header.
pub async fn connect(h: &Harness, token: &str) -> Client {
    let transport = StreamableHttpClientTransport::from_config(
        StreamableHttpClientTransportConfig::with_uri(format!("http://{}/mcp", h.addr))
            .auth_header(token),
    );
    ClientConfig::default()
        .serve_with_lifecycle(
            transport,
            ClientLifecycleMode::Auto {
                preferred_versions: vec![rmcp::model::ProtocolVersion::V_2026_07_28],
                legacy_version: Some(rmcp::model::ProtocolVersion::V_2025_11_25),
            },
        )
        .await
        .expect("the client connects")
}

/// What a tool call returned: whether it is a tool error, and its structured body.
pub struct Called {
    pub is_error: bool,
    pub body: Value,
}

pub async fn call(c: &Client, tool: &'static str, args: Value) -> Called {
    let params = CallToolRequestParams::new(tool)
        .with_arguments(args.as_object().cloned().expect("arguments are an object"));
    let res = c.call_tool(params).await.expect("the call is answered");
    let body = res
        .structured_content
        .clone()
        .expect("every result has a structured body");
    Called {
        is_error: res.is_error.unwrap_or(false),
        body,
    }
}

pub fn frame(id: &str, name: &str) -> Value {
    json!({"type": "addNode", "node": {"id": id, "type": "frame", "name": name, "w": 800, "h": 400}})
}

pub fn text(id: &str, parent: &str, content: &str) -> Value {
    json!({"type": "addNode", "node": {"id": id, "type": "text", "name": id, "parentId": parent, "text": content}})
}

pub fn mv(id: &str, x: f64, y: f64) -> Value {
    json!({"type": "updateNode", "id": id, "patch": {"x": x, "y": y}})
}
