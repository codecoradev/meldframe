//! `meldframe_http::serve`: the accept loop of the server. A client that sends its headers too slowly, or opens a
//! connection and sends nothing, is closed after the header-read timeout (issue 170), so it cannot hold one of the
//! capped connections for ever; ordinary calls and WebSocket upgrades are served as before.

mod common;
use common::*;
use meldframe_http::{CappedListener, serve};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

const HEADER_READ: Duration = Duration::from_millis(300);

async fn start() -> SocketAddr {
    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = tcp.local_addr().unwrap();
    let listener = CappedListener::new(tcp, 100, 100);
    let router = app(Fake::ok(), config());
    tokio::spawn(async move { serve(listener, router, HEADER_READ).await });
    addr
}

/// Read until the server closes the connection; how long that took, or `None` if it stayed open for `limit`.
async fn closed_within(s: &mut TcpStream, limit: Duration) -> Option<Duration> {
    let began = tokio::time::Instant::now();
    let mut buf = [0u8; 256];
    loop {
        match tokio::time::timeout_at(began + limit, s.read(&mut buf)).await {
            Err(_) => return None,
            Ok(Ok(0)) | Ok(Err(_)) => return Some(began.elapsed()),
            Ok(Ok(_)) => {}
        }
    }
}

#[tokio::test]
async fn headers_sent_too_slowly_close_the_connection() {
    let addr = start().await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(b"GET /v1/ws HTTP/1.1\r\nHost: x\r\n")
        .await
        .unwrap();
    // The rest of the headers never come.
    let took = closed_within(&mut s, Duration::from_secs(5)).await;
    let took = took.expect("the connection stayed open");
    assert!(
        took >= HEADER_READ - Duration::from_millis(50),
        "closed by the timeout, not at once: {took:?}"
    );
}

#[tokio::test]
async fn a_connection_that_sends_nothing_is_closed_too() {
    let addr = start().await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    let took = closed_within(&mut s, Duration::from_secs(5)).await;
    let took = took.expect("an idle connection stayed open");
    assert!(
        took >= HEADER_READ - Duration::from_millis(50),
        "closed by the timeout, not at once: {took:?}"
    );
}

#[tokio::test]
async fn an_ordinary_call_is_served() {
    let addr = start().await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    let body = r#"{"ops":[{"key":"k1","command":{"type":"removeNode","id":"n1"}}]}"#;
    let req = format!(
        "POST /v1/files/file_a/tools/paint HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out))
        .await
        .unwrap()
        .unwrap();
    let text = String::from_utf8_lossy(&out);
    assert!(text.starts_with("HTTP/1.1 200"), "{text}");
}

#[tokio::test]
async fn a_websocket_upgrade_still_works_and_outlives_the_header_timeout() {
    let addr = start().await;
    let mut req = format!("ws://{addr}/v1/ws").into_client_request().unwrap();
    req.headers_mut()
        .insert("authorization", format!("Bearer {TOKEN}").parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    // Once upgraded, the header timeout no longer applies: the socket stays open past it.
    tokio::time::sleep(HEADER_READ * 3).await;
    use futures_util::SinkExt;
    ws.send(tokio_tungstenite::tungstenite::Message::Ping(
        vec![1].into(),
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn each_request_carries_the_client_address() {
    use axum::extract::ConnectInfo;
    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = tcp.local_addr().unwrap();
    let router = axum::Router::new().route(
        "/who",
        axum::routing::get(
            |ConnectInfo(peer): ConnectInfo<SocketAddr>| async move { peer.to_string() },
        ),
    );
    tokio::spawn(serve(CappedListener::new(tcp, 10, 10), router, HEADER_READ));
    let mut s = TcpStream::connect(addr).await.unwrap();
    let me = s.local_addr().unwrap();
    s.write_all(b"GET /who HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut out = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out))
        .await
        .unwrap()
        .unwrap();
    let text = String::from_utf8_lossy(&out);
    assert!(text.ends_with(&me.to_string()), "{text}");
}

#[tokio::test]
async fn a_kept_alive_connection_idle_between_requests_is_closed() {
    let addr = start().await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(b"GET /v1/ws HTTP/1.1\r\nHost: x\r\n\r\n")
        .await
        .unwrap();
    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(Duration::from_secs(5), s.read(&mut buf))
        .await
        .unwrap()
        .unwrap();
    assert!(buf[..n].starts_with(b"HTTP/1.1 "), "answered");
    // No second request follows: the connection is closed after the timeout.
    let took = closed_within(&mut s, Duration::from_secs(5)).await;
    assert!(took.is_some(), "an idle kept-alive connection stayed open");
}
