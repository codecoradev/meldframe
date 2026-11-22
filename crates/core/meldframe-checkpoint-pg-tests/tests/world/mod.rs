//! The harness of the run-start tests: the real server behind REST, the submit socket and MCP on one router, the run
//! sink installed on it, a clock the test moves, a submitter that can fail part way and a store that can be slow or fail.
#![allow(dead_code, reason = "each test binary uses a part of these helpers")]

#[path = "../../../meldframe-mcp-tests/tests/common/mod.rs"]
mod common;
use checkpoint_store::{CheckpointError, CheckpointStore, Made, MemoryCheckpoints};
pub use common::*;
use futures_util::{SinkExt, StreamExt};
use meldframe_checkpoints::{Checkpoint, NewCheckpoint};
use meldframe_host::{Accepted, Op, Principal};
use meldframe_http::mcp::{McpConfig, Reader};
use meldframe_http::{Config, DevTokens, Failure, Guard, Submitter};
use meldframe_http_checkpoints::{Checkpoints, install_with};
use meldframe_server::{Server, SharedServer};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

pub const DELTA: &str = "agent-token-delta-04";
/// The gap of a run, and a clock value to start from.
pub const GAP: u64 = 300_000;
pub const T0: u64 = 1_700_000_000_000;

/// A submitter that can write the first ops of a call and then fail, as a store that fails part way does.
pub struct Cut {
    pub inner: Arc<SharedServer>,
    pub cut: AtomicUsize,
}

impl Cut {
    fn part(&self, t: &str, f: &str, w: &Principal, o: &[Op]) -> Result<(), Failure> {
        let cut = self.cut.load(Ordering::SeqCst);
        if cut == 0 {
            return Ok(());
        }
        self.inner.submit(t, f, w, o[..cut].to_vec())?;
        Err(Failure::Unavailable("test".into()))
    }
}

impl Submitter for Cut {
    fn submit(
        &self,
        t: &str,
        f: &str,
        w: &Principal,
        o: Vec<Op>,
    ) -> Result<Vec<Accepted>, Failure> {
        self.part(t, f, w, &o)?;
        self.inner.submit(t, f, w, o)
    }
    fn submit_atomic(
        &self,
        t: &str,
        f: &str,
        w: &Principal,
        o: Vec<Op>,
        g: Vec<Guard>,
    ) -> Result<Vec<Accepted>, Failure> {
        self.part(t, f, w, &o)?;
        self.inner.submit_atomic(t, f, w, o, g)
    }
}

/// The memory store with two faults the tests switch on: `slow` outlasts the call's time limit, `fail` refuses.
#[derive(Debug, Default)]
pub struct Shaky {
    pub inner: MemoryCheckpoints,
    pub slow: AtomicBool,
    pub fail: AtomicBool,
    pub finished: AtomicUsize,
}

impl CheckpointStore for Shaky {
    fn create(
        &self,
        w: &str,
        f: &str,
        k: Option<&str>,
        n: &NewCheckpoint,
        at: u64,
    ) -> Result<Made, CheckpointError> {
        if self.slow.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(700));
            self.finished.fetch_add(1, Ordering::SeqCst);
        }
        if self.fail.load(Ordering::SeqCst) {
            return Err(CheckpointError::Unavailable("test".into()));
        }
        self.inner.create(w, f, k, n, at)
    }

    fn list(&self, w: &str, f: &str) -> Result<Vec<Checkpoint>, CheckpointError> {
        self.inner.list(w, f)
    }
}

pub struct World {
    pub h: Harness,
    pub server: Arc<SharedServer>,
    pub store: Arc<Shaky>,
    pub clock: Arc<AtomicU64>,
    pub runs: Arc<Checkpoints>,
    pub cut: Arc<Cut>,
}

pub async fn start() -> World {
    build(
        Arc::new(SharedServer::new(Server::assemble())),
        Arc::new(Shaky::default()),
        T0,
    )
    .await
}

impl World {
    /// The same server and store behind a new router and a new sink: a process that restarted, with the files and the
    /// rows kept and the detector empty.
    pub async fn restarted(&self) -> World {
        build(self.server.clone(), self.store.clone(), self.now()).await
    }

    pub fn now(&self) -> u64 {
        self.clock.load(Ordering::SeqCst)
    }

    pub fn at(&self, ms: u64) {
        self.clock.store(ms, Ordering::SeqCst);
    }

    /// The rows of a file, newest first.
    pub fn rows(&self, ws: &str, file: &str) -> Vec<Checkpoint> {
        self.store.list(ws, file).unwrap()
    }
}

async fn build(server: Arc<SharedServer>, store: Arc<Shaky>, now: u64) -> World {
    let cut = Arc::new(Cut {
        inner: server.clone(),
        cut: AtomicUsize::new(0),
    });
    let spec = format!(
        "{AGENT}:ws_1:agent:alpha,{PERSON}:ws_1:user:bea,{OTHER}:ws_2:agent:gamma,{DELTA}:ws_1:agent:delta"
    );
    let tokens = Arc::new(DevTokens::parse(&spec).unwrap());
    let cfg = Config {
        timeout: Duration::from_millis(400),
        ..config()
    };
    let state = meldframe_http::shared(tokens, cut.clone(), cfg);
    let reader: Arc<dyn Reader> = server.clone();
    let clock = Arc::new(AtomicU64::new(now));
    let seen = clock.clone();
    let sink = Checkpoints::new(store.clone(), reader.clone())
        .with_clock(Arc::new(move || seen.load(Ordering::SeqCst)));
    let runs = install_with(&state, sink);
    let app = meldframe_http::routes(state.clone());
    let app = app.merge(meldframe_http::mcp::routes(
        state,
        reader,
        McpConfig::default(),
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let app = app.into_make_service_with_connect_info::<SocketAddr>();
        axum::serve(listener, app).await.unwrap();
    });
    let spy = Arc::new(Spy {
        server: server.clone(),
        submits: Default::default(),
        atomic_submits: Default::default(),
        reads: Default::default(),
    });
    World {
        h: Harness { addr, spy },
        server,
        store,
        clock,
        runs,
        cut,
    }
}

/// An op of the wire that adds a frame.
pub fn add(key: &str, id: &str) -> Value {
    json!({"key": key, "command": {"type": "addNode", "node": {"id": id, "type": "frame", "name": id}}})
}

/// An op that is refused: it updates a node that is not there.
pub fn bad(key: &str) -> Value {
    json!({"key": key, "command": {"type": "updateNode", "id": "nope", "patch": {"x": 1}}})
}

pub async fn rest(w: &World, token: &str, file: &str, ops: Vec<Value>) -> (u16, Value) {
    let url = format!("http://{}/v1/files/{file}/tools/example_edit", w.h.addr);
    let r = reqwest::Client::new()
        .post(url)
        .bearer_auth(token)
        .json(&json!({ "ops": ops }))
        .send()
        .await
        .unwrap();
    (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
}

/// One frame over the submit socket, with its answer.
pub async fn socket(w: &World, token: &str, file: &str, ops: Vec<Value>) -> Value {
    let mut req = format!("ws://{}/v1/ws", w.h.addr)
        .into_client_request()
        .unwrap();
    req.headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    let frame = json!({"id": "1", "tool": "example_edit", "file": file, "ops": ops});
    ws.send(Message::text(frame.to_string())).await.unwrap();
    let Some(Ok(Message::Text(answer))) = ws.next().await else {
        panic!("no answer");
    };
    serde_json::from_str(&answer).unwrap()
}

/// `apply_commands` over MCP: one frame per key.
pub fn apply(file: &str, key: &str, id: &str) -> Value {
    json!({"file": file, "idempotencyKey": key, "commands": [frame(id, id)]})
}
