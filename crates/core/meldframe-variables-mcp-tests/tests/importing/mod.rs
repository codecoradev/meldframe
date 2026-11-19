#![allow(dead_code, reason = "each test binary uses a part of these helpers")]
//! The harness of the `import_tokens` tests: `import_tokens` (E3.4) over MCP against the real server: CSS text becomes collections, modes, variables and aliases
//! in one atomic call through the gate, a long import comes in parts, a retry writes nothing twice, and the codes are
//! the documented ones.

// The helpers of `meldframe-mcp-tests`, by path: that crate is at its size ceiling.
#[path = "../../../meldframe-mcp-tests/tests/common/mod.rs"]
mod common;
pub use common::*;
use meldframe_http::mcp::McpConfig;
use meldframe_http::{DevTokens, Shared};
use meldframe_module::Policy;
use meldframe_server::{Server, SharedServer};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

pub const CODES: &str = include_str!("../../../../../docs/error-codes.md");

pub const THEME: &str = "@import \"tailwindcss\"; \
    :root { --background: oklch(1 0 0); --brand: var(--color-brand-600); --radius: 0.5rem; --shadow-ring: 0 0 0 1px #000; } \
    .dark { --background: oklch(0.145 0 0); --brand: var(--color-brand-400); } \
    @theme inline { --color-surface: var(--background); } \
    @theme { --color-brand-400: #60a5fa; --color-brand-600: #2563eb; }";

/// The server of `start`, with the import tool among the extras, as the binary composes it.
pub async fn start_with(policy: Option<Policy>) -> Harness {
    start_quota(policy, None).await
}

/// `start_with`, and a workspace quota of this many ops a minute when one is given (the clock does not move).
pub async fn start_quota(policy: Option<Policy>, quota: Option<u64>) -> Harness {
    let mut server = Server::assemble();
    if let Some(q) = quota {
        server = server.with_quotas(q, u64::MAX, || 0);
    }
    let spy = Arc::new(Spy {
        server: Arc::new(SharedServer::new(server)),
        submits: AtomicUsize::new(0),
        atomic_submits: AtomicUsize::new(0),
        reads: AtomicUsize::new(0),
    });
    let tokens = DevTokens::parse(&format!(
        "{AGENT}:ws_1:agent:alpha,{PERSON}:ws_1:user:bea,{OTHER}:ws_2:agent:gamma"
    ))
    .unwrap();
    let shared: Arc<Shared> = meldframe_http::shared(Arc::new(tokens), spy.clone(), config());
    let tool = meldframe_http_variables_import::tool(shared.clone(), policy);
    let mcp = McpConfig {
        extra: vec![tool],
        ..Default::default()
    };
    let app = meldframe_http::routes(shared.clone()).merge(meldframe_http::mcp::routes(
        shared,
        spy.clone(),
        mcp,
    ));
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

pub async fn start() -> Harness {
    start_with(Some(Policy::Auto)).await
}

pub async fn import(c: &Client, key: &str, css: &str, extra: Value) -> Called {
    let mut a = json!({"file": "f", "css": css, "idempotencyKey": key});
    a.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    call(c, "import_tokens", a).await
}

pub async fn variables(c: &Client, args: Value) -> Value {
    let mut a = json!({"file": "f", "limit": 200});
    a.as_object_mut()
        .unwrap()
        .extend(args.as_object().unwrap().clone());
    call(c, "get_variables", a).await.body
}

pub fn names(v: &Value) -> Vec<String> {
    v["variables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["name"].as_str().unwrap().to_owned())
        .collect()
}

pub fn long(n: usize) -> String {
    let mut t = String::from(":root {");
    for i in 0..n {
        // Every tenth is an alias of the one before it: an alias must come after its target in a part.
        if i % 10 == 9 {
            t.push_str(&format!("--v{i}: var(--v{});", i - 1));
        } else {
            t.push_str(&format!("--v{i}: {i}px;"));
        }
    }
    t.push('}');
    t
}
