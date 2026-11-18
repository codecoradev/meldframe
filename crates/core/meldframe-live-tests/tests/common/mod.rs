//! Shared by the WebSocket tests: a real server behind a real listener, a source wrapper that can stall or
//! hold single live calls, and the helpers to open sockets, read frames and submit ops over REST.
#![allow(dead_code)]

use axum::{Router, body::Body, http::Request};
use futures_util::{SinkExt, StreamExt};
use meldframe_host::Principal;
use meldframe_http::{Config, DevTokens, Failure, Rate, Submitter};
use meldframe_http_live::{LiveConfig, LiveSource, Opened, Out, live_router};
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::Command;
use meldframe_server::{Server, SharedServer, StreamId};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use tower::ServiceExt;

pub const ALPHA: &str = "dev-token-alpha-1";
pub const BETA: &str = "dev-token-beta-02";

struct Painter;
impl Module for Painter {
    fn name(&self) -> &'static str {
        "painter"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
    }
}

pub type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// The real source, with switches a test flips to make one live call slow, stuck or failing.
#[derive(Default)]
pub struct Hooks {
    /// Make the next `poll` sleep this many milliseconds before it takes frames (then it takes them).
    pub stall_next_poll_ms: AtomicU64,
    /// While set, every live call (poll, seen, resubscribe, unsubscribe) waits.
    pub stuck: AtomicBool,
    /// Calls of one kind (`"poll"`, `"seen"`) of one workspace that wait while listed.
    pub stick: Mutex<Vec<(String, &'static str)>>,
    /// While set, `subscribe` waits after it opened the stream; `held` says it is waiting.
    pub hold_subscribe: AtomicBool,
    pub held: AtomicBool,
    /// Make `resubscribe` fail with a store-like error.
    pub fail_resubscribe: AtomicBool,
    /// Live calls running now, the most at once, and all started.
    pub running: AtomicUsize,
    pub peak: AtomicUsize,
    pub started: AtomicUsize,
    /// Running now and the most at once, per workspace.
    per_ws: Mutex<HashMap<String, (usize, usize)>>,
    /// Calls started and ended, by `"kind:start"` and `"kind:end"`.
    counts: Mutex<HashMap<String, usize>>,
    /// Calls waiting now on a switch (`stuck` or `stick`), by workspace and kind.
    waiting: Mutex<HashMap<(String, &'static str), usize>>,
}

impl Hooks {
    /// Make calls of `kind` of workspace `ws` wait until [`Hooks::unstick`].
    pub fn stick(&self, ws: &str, kind: &'static str) {
        self.stick.lock().unwrap().push((ws.to_owned(), kind));
    }

    pub fn unstick(&self) {
        self.stick.lock().unwrap().clear();
        self.stuck.store(false, SeqCst);
        self.hold_subscribe.store(false, SeqCst);
    }

    /// The most calls of workspace `ws` that ran at once.
    pub fn peak_of(&self, ws: &str) -> usize {
        self.per_ws.lock().unwrap().get(ws).map_or(0, |p| p.1)
    }

    /// Calls of workspace `ws` running now.
    pub fn running_of(&self, ws: &str) -> usize {
        self.per_ws.lock().unwrap().get(ws).map_or(0, |p| p.0)
    }

    /// Calls of `kind` of workspace `ws` that wait now on a switch: a place is held for sure, not just for a
    /// moment by a call that is about to end.
    pub fn waiting_of(&self, ws: &str, kind: &'static str) -> usize {
        self.waiting
            .lock()
            .unwrap()
            .get(&(ws.to_owned(), kind))
            .copied()
            .unwrap_or(0)
    }

    /// How many calls hit `"kind:start"` or `"kind:end"`.
    pub fn count(&self, what: &str) -> usize {
        self.counts.lock().unwrap().get(what).copied().unwrap_or(0)
    }

    fn bump(&self, what: String) {
        *self.counts.lock().unwrap().entry(what).or_insert(0) += 1;
    }

    fn waits(&self, ws: &str, kind: &'static str) -> bool {
        self.stuck.load(SeqCst)
            || self
                .stick
                .lock()
                .unwrap()
                .iter()
                .any(|(w, k)| w == ws && *k == kind)
    }
}

/// Releases every held or stuck call when dropped, so a failed assertion cannot leave blocking threads
/// waiting (the runtime would never shut down).
pub struct Release(pub Arc<Hooks>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.unstick();
    }
}

struct Running<'a> {
    hooks: &'a Hooks,
    ws: String,
    kind: &'static str,
}
impl<'a> Running<'a> {
    fn enter(hooks: &'a Hooks, kind: &'static str, who: &Principal) -> Self {
        let ws = who.workspace.to_string();
        hooks.started.fetch_add(1, SeqCst);
        let now = hooks.running.fetch_add(1, SeqCst) + 1;
        hooks.peak.fetch_max(now, SeqCst);
        {
            let mut m = hooks.per_ws.lock().unwrap();
            let p = m.entry(ws.clone()).or_insert((0, 0));
            p.0 += 1;
            p.1 = p.1.max(p.0);
        }
        hooks.bump(format!("{kind}:start"));
        if hooks.waits(&ws, kind) {
            *hooks
                .waiting
                .lock()
                .unwrap()
                .entry((ws.clone(), kind))
                .or_insert(0) += 1;
            while hooks.waits(&ws, kind) {
                std::thread::sleep(Duration::from_millis(5));
            }
            if let Some(n) = hooks.waiting.lock().unwrap().get_mut(&(ws.clone(), kind)) {
                *n -= 1;
            }
        }
        Running { hooks, ws, kind }
    }
}
impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.hooks.running.fetch_sub(1, SeqCst);
        if let Some(p) = self.hooks.per_ws.lock().unwrap().get_mut(&self.ws) {
            p.0 -= 1;
        }
        self.hooks.bump(format!("{}:end", self.kind));
    }
}

pub struct Hooked {
    pub inner: Arc<SharedServer>,
    pub hooks: Arc<Hooks>,
    /// Polls queue here in turn, as callers queue for the server's lock: a stalled poll is taken before
    /// the ones behind it, so the frames it pops are not left for them.
    pub order: std::sync::Mutex<()>,
}

impl LiveSource for Hooked {
    fn subscribe(
        &self,
        who: &Principal,
        file: &str,
        from_seq: u64,
        now: u64,
    ) -> Result<Opened, Failure> {
        let opened = self.inner.subscribe(who, file, from_seq, now);
        if self.hooks.hold_subscribe.load(SeqCst) {
            self.hooks.held.store(true, SeqCst);
            while self.hooks.hold_subscribe.load(SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        opened
    }

    fn resubscribe(
        &self,
        who: &Principal,
        file: &str,
        id: StreamId,
        from_seq: u64,
        now: u64,
    ) -> Result<Opened, Failure> {
        let _r = Running::enter(&self.hooks, "resubscribe", who);
        if self.hooks.fail_resubscribe.load(SeqCst) {
            return Err(Failure::Unavailable("store said: secret detail".into()));
        }
        self.inner.resubscribe(who, file, id, from_seq, now)
    }

    fn poll(&self, who: &Principal, file: &str, id: StreamId, now: u64, max: usize) -> Vec<Out> {
        let _r = Running::enter(&self.hooks, "poll", who);
        let _turn = self.order.lock().unwrap_or_else(|p| p.into_inner());
        let stall = self.hooks.stall_next_poll_ms.swap(0, SeqCst);
        if stall > 0 {
            std::thread::sleep(Duration::from_millis(stall));
        }
        self.inner.poll(who, file, id, now, max)
    }

    fn seen(&self, who: &Principal, file: &str, id: StreamId, now: u64) {
        let _r = Running::enter(&self.hooks, "seen", who);
        self.inner.seen(who, file, id, now);
    }

    fn unsubscribe(&self, who: &Principal, file: &str, id: StreamId) {
        let _r = Running::enter(&self.hooks, "unsubscribe", who);
        self.inner.unsubscribe(who, file, id);
    }

    fn heartbeat_ms(&self) -> Option<u64> {
        self.inner.heartbeat_ms()
    }

    fn tick(&self, now: u64) -> Vec<StreamId> {
        self.inner.tick(now)
    }
}

pub struct Env {
    pub addr: SocketAddr,
    pub router: Router,
    pub shared: Arc<SharedServer>,
    pub hooks: Arc<Hooks>,
    pub now: Arc<AtomicU64>,
}

pub fn live_cfg(window: usize, queue: usize) -> meldframe_server::Config {
    meldframe_server::Config {
        window,
        queue,
        max_streams: 8,
        heartbeat_ms: 60_000,
        dead_after_ms: 120_000,
        stall_ms: 600_000,
    }
}

/// A server with these stream limits and a total cap, behind a real listener; the adapter's clock is
/// `now`, which the test moves by hand unless `real_clock`.
pub async fn env(live: meldframe_server::Config, total: usize, real_clock: bool) -> Env {
    env_with(live, total, real_clock, |_| {}).await
}

/// The adapter's settings: the shared `Config` of `meldframe-http` and the `LiveConfig` of the live route.
pub struct Cfg {
    pub http: Config,
    pub live: LiveConfig,
}

/// [`env`], with the adapter's settings adjusted by `tweak`.
pub async fn env_with(
    live: meldframe_server::Config,
    total: usize,
    real_clock: bool,
    tweak: impl FnOnce(&mut Cfg),
) -> Env {
    let server = Server::assemble_with(vec![Box::new(Painter)]).with_live(live, total);
    let shared = Arc::new(SharedServer::new(server));
    let now = Arc::new(AtomicU64::new(1_000));
    let n = now.clone();
    let mut config = Config {
        per_client: Rate {
            burst: 10_000,
            per_second: 10_000,
        },
        per_principal: Rate {
            burst: 10_000,
            per_second: 10_000,
        },
        ..Config::default()
    };
    if !real_clock {
        config.clock = Arc::new(move || n.load(SeqCst));
    }
    let mut cfg = Cfg {
        http: config,
        live: LiveConfig {
            live_tick: Duration::from_millis(10),
            live_poll: Duration::from_millis(20),
            ..LiveConfig::default()
        },
    };
    tweak(&mut cfg);
    let tokens =
        DevTokens::parse(&format!("{ALPHA}:ws_1:agent:alpha,{BETA}:ws_2:agent:beta")).unwrap();
    let submitter: Arc<dyn Submitter> = shared.clone();
    let hooks = Arc::new(Hooks::default());
    let source: Arc<dyn LiveSource> = Arc::new(Hooked {
        inner: shared.clone(),
        hooks: hooks.clone(),
        order: std::sync::Mutex::default(),
    });
    let (router, timer) = live_router(Arc::new(tokens), submitter, source, cfg.http, cfg.live);
    tokio::spawn(timer.run());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let serve = router
        .clone()
        .into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, serve).await.unwrap() });
    Env {
        addr,
        router,
        shared,
        hooks,
        now,
    }
}

pub async fn open(
    e: &Env,
    token: &str,
    path: &str,
) -> Result<Ws, tokio_tungstenite::tungstenite::Error> {
    let mut req = format!("ws://{}{path}", e.addr)
        .into_client_request()
        .unwrap();
    req.headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    tokio_tungstenite::connect_async(req).await.map(|(w, _)| w)
}

pub async fn connect(e: &Env, token: &str, after: Option<u64>) -> Ws {
    let q = after.map_or(String::new(), |a| format!("?after={a}"));
    open(e, token, &format!("/v1/live/file_a{q}"))
        .await
        .unwrap()
}

/// A socket whose receive buffer is tiny, so a client that does not read fills the path to it fast.
pub async fn connect_small_buffer(e: &Env, token: &str) -> Ws {
    let sock = tokio::net::TcpSocket::new_v4().unwrap();
    sock.set_recv_buffer_size(4096).unwrap();
    let stream = sock.connect(e.addr).await.unwrap();
    let mut req = format!("ws://{}/v1/live/file_a", e.addr)
        .into_client_request()
        .unwrap();
    req.headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    let plain = tokio_tungstenite::MaybeTlsStream::Plain(stream);
    tokio_tungstenite::client_async(req, plain).await.unwrap().0
}

/// The next JSON frame, `None` when the socket closes or nothing comes within `wait`.
pub async fn next_within(ws: &mut Ws, wait: Duration) -> Option<Value> {
    let end = tokio::time::Instant::now() + wait;
    loop {
        match tokio::time::timeout_at(end, ws.next()).await {
            Err(_) | Ok(None | Some(Err(_))) => return None,
            Ok(Some(Ok(Message::Text(t)))) => {
                return Some(serde_json::from_str(t.as_str()).unwrap());
            }
            Ok(Some(Ok(Message::Close(_)))) => return None,
            Ok(Some(Ok(_))) => {}
        }
    }
}

pub async fn next(ws: &mut Ws) -> Value {
    next_within(ws, Duration::from_secs(5))
        .await
        .expect("a frame within 5 s")
}

/// The next frame that is not a heartbeat.
pub async fn next_data(ws: &mut Ws) -> Value {
    loop {
        let m = next(ws).await;
        if m["type"] != "heartbeat" {
            return m;
        }
    }
}

/// Whether the socket closes (a close frame or the end of the stream) within 5 s, ignoring what it sends.
pub async fn is_closed(ws: &mut Ws) -> bool {
    let end = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        match tokio::time::timeout_at(end, ws.next()).await {
            Err(_) => return false,
            Ok(None | Some(Err(_) | Ok(Message::Close(_)))) => return true,
            Ok(Some(Ok(_))) => {}
        }
    }
}

/// The `seq`s of the next `n` events (heartbeats skipped).
pub async fn events(ws: &mut Ws, n: usize) -> Vec<u64> {
    let mut out = Vec::new();
    while out.len() < n {
        let m = next_data(ws).await;
        assert_eq!(m["type"], "event", "{m}");
        assert_eq!(m["room"], "file_a");
        out.push(m["seq"].as_u64().unwrap());
    }
    out
}

pub fn add(n: u64) -> Value {
    add_text(n, "")
}

/// An `addNode` op whose text node carries `text` (to make a frame as large as the test needs).
pub fn add_text(n: u64, text: &str) -> Value {
    let kind = if text.is_empty() { "frame" } else { "text" };
    let node =
        serde_json::from_value(json!({"id": format!("n{n}"), "type": kind, "text": text})).unwrap();
    let command = serde_json::to_value(Command::AddNode { node, index: None }).unwrap();
    json!({"key": format!("k{n}"), "command": command})
}

pub async fn rest_ops(e: &Env, token: &str, ops: Vec<Value>) -> u16 {
    let req = Request::post("/v1/files/file_a/tools/paint")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(json!({ "ops": ops }).to_string()))
        .unwrap();
    e.router
        .clone()
        .oneshot(req)
        .await
        .unwrap()
        .status()
        .as_u16()
}

/// Submit ops `from..=to` as one batch over REST, as `token`, on `file_a`; returns the HTTP status.
pub async fn rest(e: &Env, token: &str, from: u64, to: u64) -> u16 {
    rest_ops(e, token, (from..=to).map(add).collect()).await
}

pub async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::text(v.to_string())).await.unwrap();
}

/// Poll `ok` every 20 ms for up to 5 s.
pub async fn eventually(what: &str, ok: impl FnMut() -> bool) {
    eventually_within(what, Duration::from_secs(5), ok).await;
}

/// Poll `ok` every 20 ms for up to `limit`.
pub async fn eventually_within(what: &str, limit: Duration, mut ok: impl FnMut() -> bool) {
    let end = tokio::time::Instant::now() + limit;
    while !ok() {
        assert!(tokio::time::Instant::now() < end, "never happened: {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
