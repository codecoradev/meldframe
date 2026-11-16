//! The real server with the live socket, MCP and the upload route on one listener, an rmcp agent and a
//! fake editor.
#![allow(dead_code, reason = "each test binary uses a part of these helpers")]

use base64::Engine;
use capture::Config as RelayConfig;
use futures_util::{SinkExt, StreamExt};
use meldframe_host::Principal;
use meldframe_http::mcp::McpConfig;
use meldframe_http::{Config, DevTokens, Failure, Rate};
use meldframe_http_capture::{Capture, Options};
use meldframe_http_live::{LiveConfig, LiveSource, Opened, Out};
use meldframe_protocol::ServerMsg;
use meldframe_server::StreamId;
use meldframe_server::{Server, SharedServer};
use rmcp::{
    ClientServiceExt,
    model::{CallToolRequestParams, CallToolResult, ClientConfig},
    service::{ClientLifecycleMode, RoleClient, RunningService},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::time::Duration;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

pub const AGENT: &str = "agent-token-alpha-1";
pub const PERSON: &str = "person-token-beta-02";
pub const OTHER: &str = "other-token-gamma-03";
/// A second agent of workspace ws_1.
pub const DELTA: &str = "agent-token-delta-04";

pub const MAX_BYTES: usize = 4096;

/// A 1x1-header PNG of `len` bytes.
pub fn png(len: usize) -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    v.extend(1u32.to_be_bytes());
    v.extend(1u32.to_be_bytes());
    v.resize(len.max(33), 7);
    v
}

/// A real 1x1 PNG (signature, IHDR, IDAT, IEND).
pub fn valid_png() -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==")
        .unwrap()
}

/// The real server's live source, with a switch that makes publishing an ephemeral frame fail.
pub struct Source {
    inner: Arc<SharedServer>,
    pub fail_publish: AtomicBool,
}

impl LiveSource for Source {
    fn subscribe(
        &self,
        who: &Principal,
        file: &str,
        from: u64,
        now: u64,
    ) -> Result<Opened, Failure> {
        self.inner.subscribe(who, file, from, now)
    }
    fn resubscribe(
        &self,
        who: &Principal,
        file: &str,
        id: StreamId,
        from: u64,
        now: u64,
    ) -> Result<Opened, Failure> {
        self.inner.resubscribe(who, file, id, from, now)
    }
    fn poll(&self, who: &Principal, file: &str, id: StreamId, now: u64, max: usize) -> Vec<Out> {
        self.inner.poll(who, file, id, now, max)
    }
    fn seen(&self, who: &Principal, file: &str, id: StreamId, now: u64) {
        self.inner.seen(who, file, id, now);
    }
    fn unsubscribe(&self, who: &Principal, file: &str, id: StreamId) {
        self.inner.unsubscribe(who, file, id);
    }
    fn heartbeat_ms(&self) -> Option<u64> {
        self.inner.heartbeat_ms()
    }
    fn publish_ephemeral(&self, who: &Principal, file: &str, to: StreamId, msg: ServerMsg) -> bool {
        !self.fail_publish.load(SeqCst)
            && LiveSource::publish_ephemeral(&*self.inner, who, file, to, msg)
    }
    fn tick(&self, now: u64) -> Vec<StreamId> {
        self.inner.tick(now)
    }
}

/// What a test may change before the server starts.
pub struct Setup {
    pub relay: RelayConfig,
    pub options: Options,
    pub http: Config,
}

pub struct Harness {
    pub addr: SocketAddr,
    pub cap: Capture,
    pub source: Arc<Source>,
}

pub async fn start() -> Harness {
    start_with(10_000).await
}

pub async fn start_with(timeout_ms: u64) -> Harness {
    start_setup(|s| s.relay.timeout_ms = timeout_ms).await
}

pub async fn start_setup(tweak: impl FnOnce(&mut Setup)) -> Harness {
    let rate = Rate {
        burst: 10_000,
        per_second: 10_000,
    };
    let config = Config {
        per_client: rate,
        per_principal: rate,
        ..Config::default()
    };
    let server = Arc::new(SharedServer::new(Server::assemble()));
    let tokens = DevTokens::parse(&format!(
        "{AGENT}:ws_1:agent:alpha,{PERSON}:ws_1:user:bea,{OTHER}:ws_2:user:gamma,{DELTA}:ws_1:agent:delta"
    ))
    .unwrap();
    let mut setup = Setup {
        relay: RelayConfig {
            max_bytes: MAX_BYTES,
            max_pixels: 1_000_000,
            timeout_ms: 10_000,
            max_pending: 16,
            max_pending_per_client: 4,
            max_clients: 16,
            max_pending_per_workspace: 8,
            max_pending_per_principal: 4,
            max_clients_per_workspace: 8,
        },
        options: Options::default(),
        http: config,
    };
    tweak(&mut setup);
    let clock = setup.http.clock.clone();
    let shared = meldframe_http::shared(Arc::new(tokens), server.clone(), setup.http);
    let source = Arc::new(Source {
        inner: server.clone(),
        fail_publish: AtomicBool::new(false),
    });
    let cap = Capture::new(
        shared.clone(),
        source.clone(),
        server.clone(),
        clock.clone(),
        setup.relay,
        setup.options,
    );
    let live = LiveConfig {
        live_tick: Duration::from_millis(50),
        live_poll: Duration::from_millis(20),
        hook: Some(cap.hook()),
        ..LiveConfig::default()
    };
    let (live_routes, timer) =
        meldframe_http_live::routes(shared.clone(), source.clone(), live, clock);
    let mcp = McpConfig {
        extra: Some(cap.tool()),
        ..McpConfig::default()
    };
    let app = meldframe_http::routes(shared.clone())
        .merge(live_routes)
        .merge(meldframe_http::mcp::routes(shared.clone(), server, mcp))
        .merge(cap.routes());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(timer.run());
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    Harness { addr, cap, source }
}

pub type Agent = RunningService<RoleClient, ClientConfig>;

pub async fn agent(h: &Harness, token: &str) -> Agent {
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
        .expect("the agent connects")
}

pub async fn call(a: &Agent, tool: &'static str, args: Value) -> CallToolResult {
    let params = CallToolRequestParams::new(tool)
        .with_arguments(args.as_object().cloned().expect("arguments are an object"));
    a.call_tool(params).await.expect("the call is answered")
}

/// The error code of a failed tool call.
pub fn code(r: &CallToolResult) -> String {
    assert_eq!(r.is_error, Some(true), "{r:?}");
    r.structured_content.as_ref().unwrap()["error"]["code"]
        .as_str()
        .unwrap()
        .to_string()
}

/// The PNG bytes of a successful capture.
pub fn image(r: &CallToolResult) -> Vec<u8> {
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let img = r.content.iter().find_map(|c| c.as_image()).expect("image");
    assert_eq!(img.mime_type, "image/png");
    base64::engine::general_purpose::STANDARD
        .decode(&img.data)
        .unwrap()
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// A fake editor: a live socket on a file that announces what it can do.
pub struct Editor {
    ws: Ws,
    /// Frames that arrived while the editor was opening, besides the pong.
    pub early: Vec<Value>,
    file: String,
}

impl Editor {
    pub async fn open(h: &Harness, token: &str, file: &str, caps: &[&str]) -> Editor {
        let mut req = format!("ws://{}/v1/live/{file}", h.addr)
            .into_client_request()
            .unwrap();
        req.headers_mut()
            .insert("authorization", format!("Bearer {token}").parse().unwrap());
        let (ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
        let mut e = Editor {
            ws,
            early: Vec::new(),
            file: file.to_string(),
        };
        e.send(json!({"type": "capabilities", "room": file, "capabilities": caps}))
            .await;
        // Frames are handled in order: once the pong is back, the capabilities are registered.
        e.send(json!({"type": "ping"})).await;
        loop {
            let v = e.next().await;
            if v["type"] == "pong" {
                break;
            }
            e.early.push(v);
        }
        e
    }

    pub async fn send(&mut self, v: Value) {
        self.ws.send(Message::text(v.to_string())).await.unwrap();
    }

    pub async fn next(&mut self) -> Value {
        loop {
            let m = tokio::time::timeout(Duration::from_secs(10), self.ws.next())
                .await
                .expect("a frame arrives")
                .expect("socket open")
                .unwrap();
            if let Message::Text(t) = m {
                return serde_json::from_str(t.as_str()).unwrap();
            }
        }
    }

    /// The next capture request of this socket.
    pub async fn request(&mut self) -> Value {
        loop {
            let v = self.next().await;
            if v["type"] == "captureRequest" {
                assert_eq!(v["room"], self.file);
                return v;
            }
        }
    }

    pub async fn fail(&mut self, req: &Value, code: &str) {
        self.send(
            json!({"type": "captureFailed", "requestId": req["requestId"],
            "code": code, "message": "no"}),
        )
        .await;
    }
}

/// `POST /captures/{id}` as a client; returns the status.
pub async fn upload(
    h: &Harness,
    bearer: &str,
    req: &Value,
    token: &str,
    seq: u64,
    body: Vec<u8>,
) -> u16 {
    let id = req["requestId"].as_str().unwrap();
    reqwest::Client::new()
        .post(format!("http://{}/captures/{id}", h.addr))
        .bearer_auth(bearer)
        .header("x-capture-token", token)
        .header("x-capture-seq", seq.to_string())
        .body(body)
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

/// Upload the right way: the token from the request, its minimum sequence number.
pub async fn upload_ok(h: &Harness, bearer: &str, req: &Value, body: Vec<u8>) -> u16 {
    let token = req["uploadToken"].as_str().unwrap().to_string();
    upload(
        h,
        bearer,
        req,
        &token,
        req["minSeq"].as_u64().unwrap(),
        body,
    )
    .await
}

/// Open a raw HTTP upload with the head sent and the body left to the caller (`framing` is the
/// `Content-Length` or `Transfer-Encoding` header line).
pub async fn raw_upload(
    h: &Harness,
    bearer: &str,
    id: &str,
    token: &str,
    framing: &str,
) -> tokio::net::TcpStream {
    use tokio::io::AsyncWriteExt;
    let mut s = tokio::net::TcpStream::connect(h.addr).await.unwrap();
    let head = format!(
        "POST /captures/{id} HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {bearer}\r\nX-Capture-Token: {token}\r\nX-Capture-Seq: 0\r\n{framing}\r\nConnection: close\r\n\r\n"
    );
    s.write_all(head.as_bytes()).await.unwrap();
    s
}

/// What the server answered, read until it closes or `wait` passes.
pub async fn response(s: &mut tokio::net::TcpStream, wait: Duration) -> String {
    use tokio::io::AsyncReadExt;
    let mut out = String::new();
    let _ = tokio::time::timeout(wait, s.read_to_string(&mut out)).await;
    out
}

pub const CHUNKED: &str = "Transfer-Encoding: chunked";
