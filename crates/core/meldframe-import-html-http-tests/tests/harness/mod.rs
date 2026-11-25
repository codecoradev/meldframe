#![allow(dead_code, reason = "each test binary uses a part of these helpers")]
//! The harness of the `import_html` tests against the real server: the tool among the extras of the MCP route, as the
//! binary composes it, with the batch sink of the activity module when a test wants notes.

// The helpers of `meldframe-mcp-tests`, by path: that crate is at its size ceiling.
#[path = "../../../meldframe-mcp-tests/tests/common/mod.rs"]
mod common;
#[allow(unused_imports, reason = "each test binary uses a part of these")]
pub use activity_store::{BatchStore, MemoryBatches, Query};
pub use common::*;
use meldframe_http::mcp::McpConfig;
use meldframe_http::{DevTokens, Shared};
use meldframe_http_activity::Activity;
pub use meldframe_module::Policy;
use meldframe_server::{Server, SharedServer};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

pub struct World {
    pub h: Harness,
    pub store: Arc<MemoryBatches>,
    pub shared: Arc<Shared>,
}

/// What a test may switch: the policy of the tool, whether the server keeps notes, and a quota of ops a minute.
#[derive(Clone, Copy)]
pub struct Setup {
    pub policy: Option<Policy>,
    pub notes: bool,
    pub quota: Option<u64>,
}

impl Default for Setup {
    fn default() -> Self {
        Setup {
            policy: Some(Policy::Auto),
            notes: true,
            quota: None,
        }
    }
}

pub async fn start() -> World {
    start_with(Setup::default()).await
}

pub async fn start_with(setup: Setup) -> World {
    let mut server = Server::assemble();
    if let Some(q) = setup.quota {
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
    let store = Arc::new(MemoryBatches::default());
    if setup.notes {
        let sink = Activity::new(store.clone(), spy.clone()).sink();
        assert!(shared.batches.set(sink).is_ok());
    }
    let tool = meldframe_http_html_import::tool(shared.clone(), spy.clone(), setup.policy);
    let mcp = McpConfig {
        extra: vec![tool],
        ..Default::default()
    };
    let app = meldframe_http::routes(shared.clone()).merge(meldframe_http::mcp::routes(
        shared.clone(),
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
    World {
        h: Harness { addr, spy },
        store,
        shared,
    }
}

/// `import_html` into file `f` with the key and the arguments in `extra`.
pub async fn import(c: &Client, key: &str, html: &str, extra: Value) -> Called {
    let mut a = json!({"file": "f", "html": html, "idempotencyKey": key});
    a.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    call(c, "import_html", a).await
}

/// The nodes with these ids, as `get_nodes` answers.
pub async fn nodes(c: &Client, ids: &[&str]) -> Vec<Value> {
    let r = call(c, "get_nodes", json!({"file": "f", "ids": ids})).await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["missing"], json!([]), "{}", r.body);
    r.body["nodes"].as_array().unwrap().clone()
}

/// The ids of the nodes an answer lists, in order.
pub fn ids(r: &Called) -> Vec<String> {
    let list = r.body["nodes"].as_array().unwrap();
    list.iter()
        .map(|n| n["id"].as_str().unwrap().to_owned())
        .collect()
}

/// The sequence number of file `f` and how many top-level frames it has.
pub async fn head(c: &Client) -> (u64, usize) {
    let r = call(c, "get_tree", json!({"file": "f"})).await;
    (
        r.body["seq"].as_u64().unwrap(),
        r.body["roots"].as_array().unwrap().len(),
    )
}

pub async fn apply(c: &Client, key: &str, commands: Vec<Value>) -> Called {
    let r = call(
        c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": key, "commands": commands}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    r
}

/// A page that is one section with a heading and a paragraph: three nodes, the section being the root.
pub const HERO: &str = "<html><head><title>Hero page</title></head><body style='margin:0'>\
    <section data-name='Hero section' style='display:flex;flex-direction:column;gap:8px;padding:16px;background:#112233'>\
    <h1 style='font-size:32px;color:#ffffff'>Hello</h1><p style='color:#cccccc;margin:0'>World</p></section></body></html>";
