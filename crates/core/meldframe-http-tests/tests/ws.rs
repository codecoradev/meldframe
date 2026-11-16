//! WebSocket: `GET /v1/ws`, JSON frames `{id, tool, file, ops}`, over a real socket.

mod common;
use common::*;
use futures_util::{SinkExt, StreamExt};
use meldframe_http::Rate;
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_server::{Server, SharedServer};
use std::sync::Arc;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

struct Painter;
impl Module for Painter {
    fn name(&self) -> &'static str {
        "painter"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
    }
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn serve(
    s: Arc<dyn meldframe_http::Submitter>,
    c: meldframe_http::Config,
) -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = app(s, c).into_make_service_with_connect_info::<std::net::SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    addr
}

async fn connect(
    addr: std::net::SocketAddr,
    token: Option<&str>,
) -> Result<Ws, tokio_tungstenite::tungstenite::Error> {
    let mut req = format!("ws://{addr}/v1/ws").into_client_request().unwrap();
    if let Some(t) = token {
        req.headers_mut()
            .insert("authorization", format!("Bearer {t}").parse().unwrap());
    }
    tokio_tungstenite::connect_async(req)
        .await
        .map(|(ws, _)| ws)
}

fn frame(id: &str, tool: &str, ops: Vec<serde_json::Value>) -> Message {
    let v = serde_json::json!({"id": id, "tool": tool, "file": "file_a", "ops": ops});
    Message::text(v.to_string())
}

async fn reply(ws: &mut Ws) -> serde_json::Value {
    loop {
        match ws.next().await.unwrap().unwrap() {
            Message::Text(t) => return serde_json::from_str(t.as_str()).unwrap(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[tokio::test]
async fn the_handshake_without_a_valid_token_is_refused() {
    let fake = Fake::ok();
    let addr = serve(fake.clone(), config()).await;
    for token in [None, Some("nope")] {
        let err = connect(addr, token).await.unwrap_err();
        assert!(err.to_string().contains("401"), "{err}");
    }
    assert_eq!(fake.calls(), 0);
}

#[tokio::test]
async fn a_frame_submits_through_the_same_gate_and_the_op_is_in_the_room() {
    let server = Arc::new(SharedServer::new(Server::assemble_with(vec![Box::new(
        Painter,
    )])));
    let addr = serve(server.clone(), config()).await;
    let mut ws = connect(addr, Some(TOKEN)).await.unwrap();
    ws.send(frame("1", "paint", vec![add("k1", "a1")]))
        .await
        .unwrap();
    let v = reply(&mut ws).await;
    assert_eq!(
        (v["id"].as_str(), v["status"].as_u64()),
        (Some("1"), Some(200)),
        "{v}"
    );
    assert_eq!(v["accepted"][0]["written"], true);
    assert_eq!(
        server.with_server(|s| s.with_room("ws_1", "file_a", |r| r.nodes().len())),
        Some(1)
    );
    ws.send(frame("2", "paint", vec![add("k1", "a1")]))
        .await
        .unwrap();
    assert_eq!(reply(&mut ws).await["accepted"][0]["written"], false);
    ws.send(frame("3", "no_such_tool", vec![add("k2", "a2")]))
        .await
        .unwrap();
    let v = reply(&mut ws).await;
    assert_eq!(
        (v["status"].as_u64(), v["error"]["code"].as_str()),
        (Some(403), Some("forbidden"))
    );
}

#[tokio::test]
async fn malformed_frames_get_a_stable_error_and_the_connection_survives() {
    let fake = Fake::ok();
    let addr = serve(fake.clone(), config()).await;
    let mut ws = connect(addr, Some(TOKEN)).await.unwrap();
    for bad in [
        Message::text("nope"),
        Message::text("{\"id\": 1}"),
        Message::text("{\"id\":\"x\",\"tool\":\"paint\",\"file\":\"f\",\"ops\":[],\"extra\":1}"),
        Message::binary(vec![1, 2, 3]),
    ] {
        ws.send(bad).await.unwrap();
        let v = reply(&mut ws).await;
        assert_eq!(
            (v["status"].as_u64(), v["error"]["code"].as_str()),
            (Some(400), Some("bad_request")),
            "{v}"
        );
    }
    assert_eq!(fake.calls(), 0);
    ws.send(frame("ok", "paint", vec![add("k", "a")]))
        .await
        .unwrap();
    assert_eq!(reply(&mut ws).await["status"], 200);
}

#[tokio::test]
async fn a_limited_principal_gets_a_rate_limited_frame_before_the_frame_is_parsed() {
    let (_, clock) = manual_clock();
    let mut c = config();
    c.per_principal = Rate {
        burst: 1,
        per_second: 1,
    };
    c.clock = clock;
    let fake = Fake::ok();
    let addr = serve(fake.clone(), c).await;
    let mut ws = connect(addr, Some(TOKEN)).await.unwrap();
    ws.send(frame("1", "paint", vec![add("k", "a")]))
        .await
        .unwrap();
    assert_eq!(reply(&mut ws).await["status"], 200);
    ws.send(Message::text("garbage that would be a bad_request"))
        .await
        .unwrap();
    let v = reply(&mut ws).await;
    assert_eq!(
        (v["status"].as_u64(), v["error"]["code"].as_str()),
        (Some(429), Some("rate_limited"))
    );
    assert_eq!(fake.calls(), 1);
}

#[tokio::test]
async fn an_oversized_frame_ends_the_connection_and_not_the_server() {
    let mut c = config();
    c.max_body = 1024;
    let fake = Fake::ok();
    let addr = serve(fake.clone(), c).await;
    let mut ws = connect(addr, Some(TOKEN)).await.unwrap();
    ws.send(Message::text("x".repeat(10_000))).await.unwrap();
    let end = loop {
        match ws.next().await {
            Some(Ok(Message::Text(t))) => {
                let v: serde_json::Value = serde_json::from_str(t.as_str()).unwrap();
                break v["status"].as_u64();
            }
            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break None,
            Some(Ok(_)) => {}
        }
    };
    assert!(matches!(end, None | Some(413)));
    let mut again = connect(addr, Some(TOKEN)).await.unwrap();
    again
        .send(frame("1", "paint", vec![add("k", "a")]))
        .await
        .unwrap();
    assert_eq!(reply(&mut again).await["status"], 200);
}

fn text_status(m: &serde_json::Value) -> Option<u64> {
    m["status"].as_u64()
}

#[tokio::test]
async fn a_frame_cannot_name_a_workspace_and_a_bad_file_is_invalid() {
    let fake = Fake::ok();
    let addr = serve(fake.clone(), config()).await;
    let mut ws = connect(addr, Some(TOKEN)).await.unwrap();
    let sneaky = serde_json::json!({"id": "1", "tool": "paint", "file": "f", "workspace": "ws_2", "ops": [add("k", "a")]});
    ws.send(Message::text(sneaky.to_string())).await.unwrap();
    assert_eq!(text_status(&reply(&mut ws).await), Some(400));
    let named =
        serde_json::json!({"id": "2", "tool": "paint", "file": "a/b", "ops": [add("k", "a")]});
    ws.send(Message::text(named.to_string())).await.unwrap();
    let v = reply(&mut ws).await;
    assert_eq!(
        (text_status(&v), v["error"]["code"].as_str()),
        (Some(400), Some("invalid"))
    );
    assert_eq!(fake.calls(), 0);
}

#[tokio::test]
async fn the_address_budget_is_spent_at_the_handshake() {
    let (_, clock) = manual_clock();
    let mut c = config();
    c.per_client = Rate {
        burst: 1,
        per_second: 1,
    };
    c.clock = clock;
    let addr = serve(Fake::ok(), c).await;
    let _first = connect(addr, Some(TOKEN)).await.unwrap();
    let err = connect(addr, Some(TOKEN)).await.unwrap_err();
    assert!(err.to_string().contains("429"), "{err}");
}

#[tokio::test]
async fn a_client_that_keeps_sending_while_limited_is_disconnected() {
    let (_, clock) = manual_clock();
    let mut c = config();
    c.per_principal = Rate {
        burst: 1,
        per_second: 1,
    };
    c.ws_max_limited = 3;
    c.clock = clock;
    let addr = serve(Fake::ok(), c).await;
    let mut ws = connect(addr, Some(TOKEN)).await.unwrap();
    ws.send(frame("1", "paint", vec![add("k", "a")]))
        .await
        .unwrap();
    assert_eq!(text_status(&reply(&mut ws).await), Some(200));
    let mut limited = 0;
    for _ in 0..6 {
        if ws.send(Message::text("spam")).await.is_err() {
            break;
        }
        match ws.next().await {
            Some(Ok(Message::Text(_))) => limited += 1,
            _ => break,
        }
    }
    assert_eq!(limited, 3, "answered three times, then closed");
    let end = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while let Some(Ok(m)) = ws.next().await {
            if matches!(m, Message::Close(_)) {
                return;
            }
        }
    })
    .await;
    assert!(end.is_ok());
}

#[tokio::test]
async fn an_idle_connection_is_closed() {
    let mut c = config();
    c.ws_idle = std::time::Duration::from_millis(200);
    let addr = serve(Fake::ok(), c).await;
    let mut ws = connect(addr, Some(TOKEN)).await.unwrap();
    let ended = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match ws.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(_)) => {}
            }
        }
    })
    .await;
    assert!(ended.is_ok(), "the server closed the idle connection");
}

async fn closed_within(ws: &mut Ws, secs: u64) -> bool {
    tokio::time::timeout(std::time::Duration::from_secs(secs), async {
        loop {
            match ws.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(_)) => {}
            }
        }
    })
    .await
    .is_ok()
}

#[tokio::test]
async fn an_allowed_frame_does_not_forgive_the_limited_ones_before_it() {
    let (t, clock) = manual_clock();
    let mut c = config();
    c.per_principal = Rate {
        burst: 1,
        per_second: 1,
    };
    c.ws_max_limited = 3;
    c.clock = clock;
    let addr = serve(Fake::ok(), c).await;
    let mut ws = connect(addr, Some(TOKEN)).await.unwrap();
    ws.send(frame("1", "paint", vec![add("k", "a")]))
        .await
        .unwrap();
    assert_eq!(text_status(&reply(&mut ws).await), Some(200));
    for _ in 0..2 {
        ws.send(Message::text("spam")).await.unwrap();
        assert_eq!(text_status(&reply(&mut ws).await), Some(429));
    }
    t.fetch_add(1000, std::sync::atomic::Ordering::SeqCst);
    ws.send(frame("2", "paint", vec![add("k2", "b")]))
        .await
        .unwrap();
    assert_eq!(text_status(&reply(&mut ws).await), Some(200));
    // Two limited frames, one allowed (leaky counter: 1 left), then two more limited: closed at the second.
    ws.send(Message::text("spam")).await.unwrap();
    assert_eq!(text_status(&reply(&mut ws).await), Some(429));
    ws.send(Message::text("spam")).await.unwrap();
    assert_eq!(text_status(&reply(&mut ws).await), Some(429));
    assert!(
        closed_within(&mut ws, 2).await,
        "a reset counter would still be open"
    );
}

#[tokio::test]
async fn pings_do_not_keep_a_connection_alive() {
    let mut c = config();
    c.ws_idle = std::time::Duration::from_millis(300);
    let addr = serve(Fake::ok(), c).await;
    let mut ws = connect(addr, Some(TOKEN)).await.unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut closed = false;
    while std::time::Instant::now() < deadline && !closed {
        if ws.send(Message::Ping(vec![1].into())).await.is_err() {
            closed = true;
            break;
        }
        if let Ok(None | Some(Err(_)) | Some(Ok(Message::Close(_)))) =
            tokio::time::timeout(std::time::Duration::from_millis(100), ws.next()).await
        {
            closed = true;
        }
    }
    assert!(closed, "pings kept the idle connection open");
}

#[tokio::test]
async fn pings_spend_the_principal_budget() {
    let (_, clock) = manual_clock();
    let mut c = config();
    c.per_principal = Rate {
        burst: 1,
        per_second: 1,
    };
    c.ws_max_limited = 3;
    c.clock = clock;
    let addr = serve(Fake::ok(), c).await;
    let mut ws = connect(addr, Some(TOKEN)).await.unwrap();
    let mut closed = false;
    for _ in 0..20 {
        if ws.send(Message::Ping(vec![1].into())).await.is_err() {
            closed = true;
            break;
        }
        if let Ok(None | Some(Err(_)) | Some(Ok(Message::Close(_)))) =
            tokio::time::timeout(std::time::Duration::from_millis(100), ws.next()).await
        {
            closed = true;
            break;
        }
    }
    assert!(closed, "a ping flood was never limited");
}

#[tokio::test]
async fn a_client_that_never_reads_is_dropped_once_a_send_times_out() {
    let mut c = config();
    c.ws_send_timeout = std::time::Duration::from_millis(300);
    c.max_body = 1 << 20;
    let reason = "x".repeat(4000);
    let fake = Fake::failing(meldframe_http::Failure::Refused(
        meldframe_host::Refusal::Invalid { reason },
    ));
    let addr = serve(fake, c).await;
    let mut ws = connect(addr, Some(TOKEN)).await.unwrap();
    // Send frames and never read: the server's replies fill the socket buffers.
    for i in 0..6000 {
        if ws
            .send(frame(&i.to_string(), "paint", vec![add("k", "a")]))
            .await
            .is_err()
        {
            break;
        }
    }
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    assert!(
        closed_within(&mut ws, 10).await,
        "the server still holds a connection nobody reads"
    );
}
