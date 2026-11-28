//! `meldframe_http::serve`: the accept loop of the server. A client that sends its headers too slowly, or opens a
//! connection and sends nothing, is closed after the header-read timeout (issue 170), so it cannot hold one of the
//! capped connections for ever; ordinary calls and WebSocket upgrades are served as before.

mod http_common;
use http_common::*;
use meldframe_http::{CappedListener, real_client, serve};
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
    tokio::spawn(async move { serve(listener, router, HEADER_READ, Vec::new()).await });
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
    tokio::spawn(serve(
        CappedListener::new(tcp, 10, 10),
        router,
        HEADER_READ,
        Vec::new(),
    ));
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

fn xff(v: &[&str]) -> axum::http::HeaderMap {
    let mut h = axum::http::HeaderMap::new();
    for x in v {
        h.append("x-forwarded-for", x.parse().unwrap());
    }
    h
}

#[test]
fn the_client_behind_a_trusted_proxy_is_the_rightmost_address_the_proxies_did_not_add() {
    let proxy: SocketAddr = "10.0.0.1:5000".parse().unwrap();
    let trusted = ["10.0.0.1".parse().unwrap(), "10.0.0.2".parse().unwrap()];
    let ip = |h, peer: SocketAddr| real_client(peer, &h, &trusted).ip().to_string();
    // The proxy appended the address it saw; whatever the client wrote before it is not believed.
    assert_eq!(ip(xff(&["1.2.3.4"]), proxy), "1.2.3.4");
    assert_eq!(ip(xff(&["6.6.6.6, 1.2.3.4"]), proxy), "1.2.3.4");
    // Two proxies in a row: the second one's own entry is skipped.
    assert_eq!(ip(xff(&["6.6.6.6, 1.2.3.4, 10.0.0.2"]), proxy), "1.2.3.4");
    assert_eq!(
        ip(xff(&["6.6.6.6", "1.2.3.4"]), proxy),
        "1.2.3.4",
        "headers in order"
    );
    assert_eq!(ip(xff(&["2001:db8::7"]), proxy), "2001:db8::7");
    // No header, or one that does not parse: the proxy itself is the client.
    assert_eq!(ip(xff(&[]), proxy), "10.0.0.1");
    assert_eq!(ip(xff(&["not an address"]), proxy), "10.0.0.1");
    // An IPv4 proxy seen through an IPv6 socket, and a mapped client address, are their IPv4 selves.
    let mapped: SocketAddr = "[::ffff:10.0.0.1]:5000".parse().unwrap();
    assert_eq!(ip(xff(&["1.2.3.4"]), mapped), "1.2.3.4");
    assert_eq!(ip(xff(&["::ffff:1.2.3.4"]), proxy), "1.2.3.4");
    assert_eq!(ip(xff(&["1.2.3.4, ::ffff:10.0.0.2"]), proxy), "1.2.3.4");
    // Ports, brackets and empty entries some proxies write.
    assert_eq!(ip(xff(&["1.2.3.4:5678"]), proxy), "1.2.3.4");
    assert_eq!(ip(xff(&["[2001:db8::7]:443"]), proxy), "2001:db8::7");
    assert_eq!(ip(xff(&["1.2.3.4, "]), proxy), "1.2.3.4");
    // A peer that is not a trusted proxy is the client, whatever it says.
    let direct: SocketAddr = "5.5.5.5:1".parse().unwrap();
    assert_eq!(ip(xff(&["1.2.3.4"]), direct), "5.5.5.5");
}

#[tokio::test]
async fn two_clients_behind_one_trusted_proxy_are_two_addresses() {
    use axum::extract::ConnectInfo;
    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = tcp.local_addr().unwrap();
    let router = axum::Router::new().route(
        "/who",
        axum::routing::get(|ConnectInfo(peer): ConnectInfo<SocketAddr>| async move {
            peer.ip().to_string()
        }),
    );
    let trusted = vec!["127.0.0.1".parse().unwrap()];
    tokio::spawn(serve(
        CappedListener::new(tcp, 10, 10),
        router,
        HEADER_READ,
        trusted,
    ));
    let mut seen = Vec::new();
    for client in ["1.2.3.4", "5.6.7.8"] {
        let mut s = TcpStream::connect(addr).await.unwrap();
        let req = format!(
            "GET /who HTTP/1.1\r\nHost: x\r\nX-Forwarded-For: {client}\r\nConnection: close\r\n\r\n"
        );
        s.write_all(req.as_bytes()).await.unwrap();
        let mut out = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out))
            .await
            .unwrap()
            .unwrap();
        seen.push(
            String::from_utf8_lossy(&out)
                .rsplit("\r\n")
                .next()
                .unwrap()
                .to_string(),
        );
    }
    assert_eq!(seen, ["1.2.3.4", "5.6.7.8"]);
}
