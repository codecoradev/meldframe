//! The harness of the run-start tests: the real server behind REST, the submit socket and MCP on one router, the run
//! sink installed on it, a clock the test moves, a submitter that can fail part way and a store that can be slow or fail.
#![allow(dead_code, reason = "each test binary uses a part of these helpers")]

#[path = "../../../meldframe-mcp-tests/tests/common/mod.rs"]
mod common;
use checkpoint_store::{CheckpointError, CheckpointStore, Made, MemoryCheckpoints};
pub use common::*;
use futures_util::{SinkExt, StreamExt};
use meldframe_checkpoints::{Checkpoint, NewCheckpoint};
use meldframe_host::{Accepted, Op, Principal, StoredOp};
use meldframe_http::mcp::{McpConfig, Reader};
use meldframe_http::{Config, DevTokens, Failure, Guard, Rate, Submitter};
use meldframe_http_checkpoints::{
    Checkpoints, Gate, Log, Past, Publish, StateLimits, install_with,
};
use meldframe_module::Policy;
use meldframe_protocol::Doc;
use meldframe_server::{Server, SharedServer};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
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

/// The reads of the state route, counted: openings of the file, reads of the snapshot, the numbers the log was paged from.
/// What a `CountPast` does to the log it reads, to make a log the server cannot work out.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The first op it pages is an update of a node that is not there.
    Corrupt,
    /// It pages nothing: the log ends before the checkpoint.
    Short,
}

pub struct CountPast {
    pub fault: Mutex<Option<Fault>>,
    pub inner: Arc<SharedServer>,
    pub opened: AtomicUsize,
    pub snapshots: AtomicUsize,
    pub afters: Mutex<Vec<u64>>,
}

impl CountPast {
    pub fn pages(&self) -> usize {
        self.afters.lock().unwrap().len()
    }

    pub fn reset(&self) {
        self.opened.store(0, Ordering::SeqCst);
        self.snapshots.store(0, Ordering::SeqCst);
        self.afters.lock().unwrap().clear();
    }
}

struct Counted<'a> {
    log: &'a dyn Log,
    past: &'a CountPast,
}

impl Log for Counted<'_> {
    fn snapshot(&self) -> Result<Option<(u64, Doc)>, Failure> {
        self.past.snapshots.fetch_add(1, Ordering::SeqCst);
        self.log.snapshot()
    }

    fn ops_after(&self, after: u64, limit: usize) -> Result<Vec<StoredOp>, Failure> {
        self.past.afters.lock().unwrap().push(after);
        let mut ops = self.log.ops_after(after, limit)?;
        match *self.past.fault.lock().unwrap() {
            Some(Fault::Short) => ops.clear(),
            Some(Fault::Corrupt) => {
                if let Some(first) = ops.first_mut() {
                    first.command = serde_json::from_value(
                        serde_json::json!({"type": "updateNode", "id": "nope", "patch": {"x": 1}}),
                    )
                    .unwrap();
                }
            }
            None => {}
        }
        Ok(ops)
    }
}

impl Past for CountPast {
    fn with_log(
        &self,
        w: &Principal,
        f: &str,
        run: &mut dyn FnMut(&dyn Log),
    ) -> Result<(), Failure> {
        self.opened.fetch_add(1, Ordering::SeqCst);
        self.inner
            .with_log(w, f, &mut |log| run(&Counted { log, past: self }))
    }
}

pub struct World {
    pub h: Harness,
    pub past: Arc<CountPast>,
    pub server: Arc<SharedServer>,
    pub store: Arc<Shaky>,
    pub clock: Arc<AtomicU64>,
    pub runs: Arc<Checkpoints>,
    pub cut: Arc<Cut>,
}

pub async fn start() -> World {
    start_gate(&["user:bea"], Some(Policy::Auto)).await
}

/// As [`start`], with the actors that may make a checkpoint and the registry's policy for `checkpoint_create`.
pub async fn start_gate(may: &[&str], policy: Option<Policy>) -> World {
    let spec = Spec {
        may: may.iter().map(|a| (*a).to_owned()).collect(),
        policy,
        ..Spec::default()
    };
    let server = Arc::new(SharedServer::new(Server::assemble()));
    build(server, Arc::new(Shaky::default()), T0, spec).await
}

/// What a world is built with: who may make or read, the registry's policy for each call, the state bounds and budget.
#[derive(Clone)]
pub struct Spec {
    pub may: Vec<String>,
    pub policy: Option<Policy>,
    pub state_policy: Option<Policy>,
    pub limits: StateLimits,
    pub per_principal: Option<Rate>,
    pub timeout: Duration,
}

impl Default for Spec {
    fn default() -> Self {
        Spec {
            may: vec!["user:bea".to_owned()],
            policy: Some(Policy::Auto),
            state_policy: Some(Policy::Auto),
            limits: StateLimits::default(),
            per_principal: None,
            timeout: Duration::from_millis(400),
        }
    }
}

/// A world on a server the test built, with a spec of its own.
pub async fn start_on(server: Arc<SharedServer>, spec: Spec) -> World {
    build(server, Arc::new(Shaky::default()), T0, spec).await
}

impl World {
    /// The same server and store behind a new router and a new sink: a process that restarted, with the files and the
    /// rows kept and the detector empty.
    pub async fn restarted(&self) -> World {
        let (server, store) = (self.server.clone(), self.store.clone());
        build(server, store, self.now(), Spec::default()).await
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

async fn build(server: Arc<SharedServer>, store: Arc<Shaky>, now: u64, spec: Spec) -> World {
    let cut = Arc::new(Cut {
        inner: server.clone(),
        cut: AtomicUsize::new(0),
    });
    let accounts = format!(
        "{AGENT}:ws_1:agent:alpha,{PERSON}:ws_1:user:bea,{OTHER}:ws_2:agent:gamma,{DELTA}:ws_1:agent:delta"
    );
    let tokens = Arc::new(DevTokens::parse(&accounts).unwrap());
    let cfg = Config {
        timeout: spec.timeout,
        per_principal: spec.per_principal.unwrap_or(config().per_principal),
        ..config()
    };
    let state = meldframe_http::shared(tokens, cut.clone(), cfg);
    let reader: Arc<dyn Reader> = server.clone();
    let clock = Arc::new(AtomicU64::new(now));
    let seen = clock.clone();
    let sink = Checkpoints::new(store.clone(), reader.clone())
        .with_state_limits(spec.limits)
        .with_clock(Arc::new(move || seen.load(Ordering::SeqCst)));
    let live = server.clone();
    let publish: Publish = Arc::new(move |w, f, m| live.publish_with_headroom(w, f, m));
    let listed = spec.may;
    let may = Arc::new(move |actor: &str| listed.iter().any(|a| a == actor));
    let past = Arc::new(CountPast {
        fault: Mutex::new(None),
        inner: server.clone(),
        opened: AtomicUsize::new(0),
        snapshots: AtomicUsize::new(0),
        afters: Mutex::new(Vec::new()),
    });
    let gate = Gate {
        may,
        policy: spec.policy,
        state_policy: spec.state_policy,
        publish,
        past: past.clone(),
    };
    let (runs, routes) = install_with(&state, sink, gate);
    let app = meldframe_http::routes(state.clone()).merge(routes);
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
        past,
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

pub async fn get(w: &World, token: Option<&str>, path: &str) -> (u16, Value) {
    let mut req = reqwest::Client::new().get(format!("http://{}{path}", w.h.addr));
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    let r = req.send().await.unwrap();
    (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
}

/// A POST of `body` as it is, so a test can send what is not JSON.
pub async fn post(w: &World, token: Option<&str>, path: &str, body: &str) -> (u16, Value) {
    let mut req = reqwest::Client::new()
        .post(format!("http://{}{path}", w.h.addr))
        .header("content-type", "application/json")
        .body(body.to_owned());
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    let r = req.send().await.unwrap();
    (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
}
