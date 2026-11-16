//! Overload, a panicking call, file names and the shape of a token on the wire.

mod common;
use axum::{body::Body, http::Request};
use common::*;
use meldframe_http::Failure;
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_server::{Server, SharedServer};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tower::ServiceExt;

struct Painter;
impl Module for Painter {
    fn name(&self) -> &'static str {
        "painter"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
    }
}

fn call(
    router: &axum::Router,
    token: &'static str,
) -> tokio::task::JoinHandle<(u16, serde_json::Value)> {
    let r = router.clone();
    tokio::spawn(async move {
        json(
            r.oneshot(post("paint", Some(token), body(vec![add("k", "a")])))
                .await
                .unwrap(),
        )
        .await
    })
}

#[tokio::test]
async fn the_call_over_the_in_flight_cap_gets_503_at_once_and_capacity_comes_back() {
    let mut c = config();
    c.max_in_flight = 2;
    c.max_per_principal = 10;
    let router = app(Fake::slow(Duration::from_millis(300)), c);
    let (a, b) = (call(&router, TOKEN), call(&router, OTHER));
    tokio::time::sleep(Duration::from_millis(60)).await;
    let t = Instant::now();
    let (status, v) = call(&router, TOKEN).await.unwrap();
    assert_eq!(
        (
            status,
            v["error"]["code"].as_str(),
            v["error"]["retryable"].as_bool()
        ),
        (503, Some("unavailable"), Some(true))
    );
    assert!(
        t.elapsed() < Duration::from_millis(150),
        "refused at once, not queued"
    );
    assert_eq!((a.await.unwrap().0, b.await.unwrap().0), (200, 200));
    assert_eq!(
        call(&router, TOKEN).await.unwrap().0,
        200,
        "the slots were released"
    );
}

#[tokio::test]
async fn one_principal_cannot_take_every_slot() {
    let mut c = config();
    c.max_in_flight = 10;
    c.max_per_principal = 1;
    let router = app(Fake::slow(Duration::from_millis(300)), c);
    let first = call(&router, TOKEN);
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(call(&router, TOKEN).await.unwrap().0, 503);
    assert_eq!(call(&router, OTHER).await.unwrap().0, 200);
    assert_eq!(first.await.unwrap().0, 200);
    assert_eq!(call(&router, TOKEN).await.unwrap().0, 200);
}

#[tokio::test]
async fn a_timed_out_call_keeps_its_slot_until_the_blocking_job_ends() {
    let mut c = config();
    c.max_in_flight = 1;
    c.timeout = Duration::from_millis(50);
    let slow = Fake::slow(Duration::from_millis(400));
    let router = app(slow.clone(), c);
    assert_eq!(call(&router, TOKEN).await.unwrap().0, 504);
    assert_eq!(
        call(&router, TOKEN).await.unwrap().0,
        503,
        "the abandoned job still runs"
    );
    tokio::time::sleep(Duration::from_millis(450)).await;
    slow.set_delay(Duration::ZERO);
    assert_eq!(call(&router, TOKEN).await.unwrap().0, 200);
}

#[tokio::test]
async fn after_a_panic_with_the_lock_held_the_rooms_come_back_from_the_store() {
    let server = Arc::new(SharedServer::new(Server::assemble_with(vec![Box::new(
        Painter,
    )])));
    let router = app(server.clone(), config());
    let add_one = |key: &'static str, id: &'static str| {
        let r = router.clone();
        async move {
            r.oneshot(post("paint", Some(TOKEN), body(vec![add(key, id)])))
                .await
                .unwrap()
                .status()
        }
    };
    assert_eq!(add_one("k1", "a1").await, 200);
    let s = server.clone();
    let died =
        std::thread::spawn(move || s.with_server(|_| panic!("a call died with the lock held")));
    assert!(died.join().is_err());
    let open = server.with_server(|s| s.with_room("ws_1", "file_a", |r| r.nodes().len()));
    assert_eq!(open, None, "recovering the lock closed every room");
    assert_eq!(add_one("k2", "a2").await, 200);
    let nodes = server.with_server(|s| s.with_room("ws_1", "file_a", |r| r.nodes().len()));
    assert_eq!(
        nodes,
        Some(2),
        "a1 was restored from the store, a2 was added"
    );
    let again = server.with_server(|s| s.with_room("ws_1", "file_a", |r| r.nodes().len()));
    assert_eq!(again, Some(2), "recovery happens once, not on every call");
}

#[tokio::test]
async fn file_names_the_server_should_not_see_are_invalid() {
    let fake = Fake::ok();
    let long = "a".repeat(129);
    for name in [long.as_str(), "a%00b", "a%0Ab", "a%2Fb", "a%7Fb"] {
        let req = Request::post(format!("/v1/files/{name}/tools/paint"))
            .header("authorization", format!("Bearer {TOKEN}"))
            .body(Body::from(body(vec![add("k", "a")])))
            .unwrap();
        let (status, v) = json(app(fake.clone(), config()).oneshot(req).await.unwrap()).await;
        assert_eq!(
            (status, v["error"]["code"].as_str()),
            (400, Some("invalid")),
            "{name}"
        );
    }
    assert_eq!(fake.calls(), 0);
}

#[tokio::test]
async fn data_the_store_can_never_hold_is_not_retryable() {
    use meldframe_host::{RoomError, StoreError};
    use meldframe_rooms::RegistryError;
    use meldframe_server::{SubmitError, failure_of};
    let wrap = |m: &str| {
        SubmitError::Rooms(RegistryError::Room(RoomError::Store(
            StoreError::Unavailable(m.into()),
        )))
    };
    let marked = format!("{} {}", meldframe_server::PERMANENT_STORE_ERROR, "nul");
    assert!(matches!(failure_of(&wrap(&marked)), Failure::Permanent(_)));
    // The marker counts only at the start: text a caller controls inside a message does not make it permanent.
    let spoof = format!("store down: {marked}");
    assert!(matches!(failure_of(&wrap(&spoof)), Failure::Unavailable(_)));
    assert!(matches!(
        failure_of(&wrap("store down")),
        Failure::Unavailable(_)
    ));
    let text = "data refused, retrying cannot succeed: nul";
    let res = app(Fake::failing(Failure::Permanent(text.into())), config())
        .oneshot(post("paint", Some(TOKEN), body(vec![add("k", "a")])))
        .await
        .unwrap();
    let (status, v) = json(res).await;
    assert_eq!(
        (status, v["error"]["retryable"].as_bool()),
        (400, Some(false)),
        "{v}"
    );
}

fn req_with(auth: &[&str], uri: &str) -> Request<Body> {
    let mut r = Request::post(uri);
    for a in auth {
        r = r.header("authorization", *a);
    }
    r.body(Body::from(body(vec![add("k", "a")]))).unwrap()
}

#[tokio::test]
async fn only_one_well_formed_bearer_header_authenticates() {
    let uri = "/v1/files/f/tools/paint";
    let good = format!("Bearer {TOKEN}");
    let lower = format!("bearer {TOKEN}");
    let upper = format!("BEARER  {TOKEN}");
    let basic = format!("Basic {TOKEN}");
    let query = format!("{uri}?token={TOKEN}");
    let query2 = format!("{uri}?access_token={TOKEN}");
    let cases: Vec<(Vec<&str>, &str, u16)> = vec![
        (vec![good.as_str()], uri, 200),
        (vec![lower.as_str()], uri, 200),
        (vec![upper.as_str()], uri, 200),
        (vec!["Bearer "], uri, 401),
        (vec!["Bearer"], uri, 401),
        (vec![TOKEN], uri, 401),
        (vec![basic.as_str()], uri, 401),
        (vec![good.as_str(), good.as_str()], uri, 401),
        (vec!["Bearer wrong-wrong-wrong-1", good.as_str()], uri, 401),
        (vec![], query.as_str(), 401),
        (vec![], query2.as_str(), 401),
    ];
    for (headers, uri, want) in cases {
        let res = app(Fake::ok(), config())
            .oneshot(req_with(&headers, uri))
            .await
            .unwrap();
        assert_eq!(res.status().as_u16(), want, "{headers:?} {uri}");
    }
}

#[tokio::test]
async fn no_error_body_repeats_a_token() {
    let secret = "secret-wrong-token-xyz";
    let wrong = app(Fake::ok(), config())
        .oneshot(post("paint", Some(secret), body(vec![])))
        .await
        .unwrap();
    let (_, v) = json(wrong).await;
    assert!(!v.to_string().contains(secret));
    let bad = app(Fake::ok(), config())
        .oneshot(post("paint", Some(TOKEN), b"nope".to_vec()))
        .await
        .unwrap();
    let (status, v) = json(bad).await;
    assert_eq!(status, 400);
    assert!(!v.to_string().contains(TOKEN));
}

fn watched(polled: Arc<AtomicBool>) -> Body {
    Body::from_stream(futures_util::stream::poll_fn(move |_| {
        polled.store(true, Ordering::SeqCst);
        std::task::Poll::Ready(None::<Result<Vec<u8>, std::io::Error>>)
    }))
}

#[tokio::test]
async fn a_declared_length_over_the_limit_is_refused_without_reading_the_body() {
    let mut c = config();
    c.max_body = 64;
    let polled = Arc::new(AtomicBool::new(false));
    let req = Request::post("/v1/files/f/tools/paint")
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("content-length", "100000")
        .body(watched(polled.clone()))
        .unwrap();
    let (status, v) = json(app(Fake::ok(), c).oneshot(req).await.unwrap()).await;
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (413, Some("too_large"))
    );
    assert!(!polled.load(Ordering::SeqCst));
}

#[tokio::test]
async fn a_body_that_never_finishes_times_out() {
    let mut c = config();
    c.body_timeout = Duration::from_millis(100);
    let fake = Fake::ok();
    let stalled = Body::from_stream(futures_util::stream::pending::<
        Result<Vec<u8>, std::io::Error>,
    >());
    let req = Request::post("/v1/files/f/tools/paint")
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(stalled)
        .unwrap();
    let (status, v) = json(app(fake.clone(), c).oneshot(req).await.unwrap()).await;
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (408, Some("request_timeout"))
    );
    assert_eq!(fake.calls(), 0);
}

#[tokio::test]
async fn names_with_tricks_in_them_are_invalid_and_ordinary_names_pass() {
    let bad_files = [
        "%2E",
        "%2E%2E",
        "%20%20",
        "%5C",
        "%E2%80%AE",
        "%E2%81%A6",
        "%E2%80%8B",
        "%E2%80%A8",
        "%E2%80%A9",
        "%EF%BB%BF",
        "a%5Cb",
    ];
    let long_tool = "t".repeat(65);
    let bad_tools = [
        "bad%20tool",
        "a%2Fb",
        long_tool.as_str(),
        "t%C3%A9",
        "x%00",
        "a%3Bb",
    ];
    let fake = Fake::ok();
    let mut cases: Vec<(String, String)> = bad_files
        .iter()
        .map(|f| ((*f).into(), "paint".into()))
        .collect();
    cases.extend(bad_tools.iter().map(|t| ("file_a".into(), (*t).into())));
    for (file, tool) in cases {
        let req = Request::post(format!("/v1/files/{file}/tools/{tool}"))
            .header("authorization", format!("Bearer {TOKEN}"))
            .body(Body::from(body(vec![add("k", "a")])))
            .unwrap();
        let (status, v) = json(app(fake.clone(), config()).oneshot(req).await.unwrap()).await;
        assert_eq!(
            (status, v["error"]["code"].as_str()),
            (400, Some("invalid")),
            "{file} {tool}"
        );
    }
    assert_eq!(fake.calls(), 0);
    for (file, tool) in [
        ("my-file_1.fig", "paint"),
        ("a%20b", "t.v-1_x"),
        ("%C3%A9", "paint"),
    ] {
        let req = Request::post(format!("/v1/files/{file}/tools/{tool}"))
            .header("authorization", format!("Bearer {TOKEN}"))
            .body(Body::from(body(vec![add("k", "a")])))
            .unwrap();
        assert_eq!(
            app(fake.clone(), config())
                .oneshot(req)
                .await
                .unwrap()
                .status(),
            200,
            "{file} {tool}"
        );
    }
}
