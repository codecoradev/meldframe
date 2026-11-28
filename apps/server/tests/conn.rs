//! A cap on open connections: the listener stops accepting at the cap and resumes when one closes.

mod http_common;
use axum::serve::ListenerExt;
use http_common::*;
use meldframe_http::CappedListener;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::test]
async fn a_connection_over_the_cap_waits_until_one_closes() {
    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = tcp.local_addr().unwrap();
    let listener = CappedListener::new(tcp, 1, 10).tap_io(|_| {});
    let router = app(Fake::ok(), config()).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

    let first = TcpStream::connect(addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut second = TcpStream::connect(addr).await.unwrap();
    second
        .write_all(b"GET /v1/ws HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut buf = vec![0u8; 64];
    let waited = tokio::time::timeout(Duration::from_millis(300), second.read(&mut buf)).await;
    assert!(waited.is_err(), "served over the cap");
    drop(first);
    let n = tokio::time::timeout(Duration::from_secs(3), second.read(&mut buf))
        .await
        .unwrap()
        .unwrap();
    assert!(
        buf[..n].starts_with(b"HTTP/1.1 "),
        "answered once a slot was free"
    );
}

#[tokio::test]
async fn one_address_cannot_hold_more_than_its_share_of_connections() {
    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = tcp.local_addr().unwrap();
    let listener = CappedListener::new(tcp, 100, 2).tap_io(|_| {});
    let router = app(Fake::ok(), config()).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

    let a = TcpStream::connect(addr).await.unwrap();
    let b = TcpStream::connect(addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut third = TcpStream::connect(addr).await.unwrap();
    let mut buf = [0u8; 8];
    let end = tokio::time::timeout(Duration::from_secs(2), third.read(&mut buf)).await;
    assert!(
        matches!(end, Ok(Ok(0)) | Ok(Err(_))),
        "the third connection of one address is closed: {end:?}"
    );
    drop(a);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut again = TcpStream::connect(addr).await.unwrap();
    again
        .write_all(b"GET /v1/ws HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let n = tokio::time::timeout(Duration::from_secs(2), again.read(&mut [0u8; 64]))
        .await
        .unwrap()
        .unwrap();
    assert!(n > 0, "a slot was free again");
    drop(b);
}
