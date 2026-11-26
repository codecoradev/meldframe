#![allow(dead_code, reason = "each test binary uses a part of these helpers")]
//! The harness of the `import_html` tests: the real server behind the MCP router with the import tool (and
//! `export_frame`, for the round trip) among the extras, as the binary composes them, and the batch headers kept in
//! memory when a test asks for them.

// The helpers of the other tests of this crate.
#[path = "../common/mod.rs"]
mod common;
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

pub const CODES: &str = include_str!("../../../../../docs/error-codes.md");
pub const CATALOGUE: &str = include_str!("../../../../../docs/mcp-tools.json");
pub const SKILL: &str = include_str!("../../../../../skills/meldframe/SKILL.md");

/// What a test asks of the server it starts.
#[derive(Clone, Copy)]
pub struct Opts {
    /// The policy the tool is served under (`None`: the module is not there).
    pub policy: Option<Policy>,
    /// Batch headers and notes are kept (the activity module).
    pub notes: bool,
    /// A workspace quota in ops a minute.
    pub quota: Option<u64>,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            policy: Some(Policy::Auto),
            notes: false,
            quota: None,
        }
    }
}

pub struct World {
    pub h: Harness,
    pub store: Arc<MemoryBatches>,
}

pub async fn start_with(o: Opts) -> World {
    let mut server = Server::assemble();
    if let Some(q) = o.quota {
        server = server.with_quotas(q, u64::MAX, || 0);
    }
    let server = Arc::new(SharedServer::new(server));
    let spy = Arc::new(Spy {
        server: server.clone(),
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
    if o.notes {
        let activity = Activity::with_live(store.clone(), spy.clone(), None);
        assert!(shared.batches.set(activity.sink()).is_ok());
    }
    let mcp = McpConfig {
        extra: vec![
            meldframe_http_html_import::tool(shared.clone(), spy.clone(), o.policy),
            meldframe_http_export::tool(shared.clone(), server.clone(), Some(Policy::Auto)),
        ],
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
    World {
        h: Harness { addr, spy },
        store,
    }
}

pub async fn start() -> World {
    start_with(Opts::default()).await
}

/// A fragment of a page: a card of 300 px with a heading, a paragraph and a button.
pub const CARD: &str = "<style>body{margin:0}</style><div id=card style=\"width:300px;padding:16px;background:#f5f5f5\"><h2 style=\"margin:0\">Title</h2><p style=\"margin:0\">Some text</p><button style=\"padding:8px\">Go</button></div>";

pub async fn import(c: &Client, key: &str, html: &str, extra: Value) -> Called {
    let mut a = json!({"file": "f", "html": html, "idempotencyKey": key});
    a.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    call(c, "import_html", a).await
}

/// The nodes of the file `f`, flat, by `get_nodes` on the ids a tree names.
pub async fn tree(c: &Client) -> Value {
    call(c, "get_tree", json!({"file": "f"})).await.body
}

pub async fn node(c: &Client, id: &str) -> Value {
    let r = call(c, "get_nodes", json!({"file": "f", "ids": [id]})).await;
    r.body["nodes"][0].clone()
}

pub fn headers(w: &World, ws: &str, file: &str) -> Vec<meldframe_activity::Header> {
    let q = Query {
        before: None,
        after: None,
        limit: 100,
    };
    w.store.list(ws, file, &q).unwrap().headers
}

/// An HTML document of `n` nodes (the page and `n - 1` boxes).
pub fn many(n: usize) -> String {
    let items: String = (0..n.saturating_sub(1))
        .map(|i| format!("<div data-name=\"box{i}\" style=\"width:10px;height:10px\"></div>"))
        .collect();
    format!("<style>body{{margin:0}}</style><div style=\"width:200px\">{items}</div>")
}

/// The number of nodes a `get_tree` answer lists.
pub fn count(tree: &Value) -> usize {
    fn walk(v: &Value) -> usize {
        1 + v["children"]
            .as_array()
            .map_or(0, |c| c.iter().map(walk).sum())
    }
    tree["roots"].as_array().unwrap().iter().map(walk).sum()
}
