//! Rate limiting happens before the body is read or parsed.

mod common;
use axum::extract::ConnectInfo;
use axum::{body::Body, http::Request};
use common::*;
use meldframe_http::{Rate, RateLimiter};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tower::ServiceExt;

fn from(ip: &str, mut r: Request<Body>) -> Request<Body> {
    let addr: SocketAddr = format!("{ip}:4000").parse().unwrap();
    r.extensions_mut().insert(ConnectInfo(addr));
    r
}

#[test]
fn a_bucket_refills_with_the_clock_and_never_above_its_burst() {
    let (t, clock) = manual_clock();
    let lim = RateLimiter::new(
        Rate {
            burst: 2,
            per_second: 1,
        },
        clock,
    );
    assert!(lim.check("a").is_ok());
    assert!(lim.check("a").is_ok());
    let wait = lim.check("a").unwrap_err();
    assert!(wait.as_millis() > 0 && wait.as_millis() <= 1000);
    assert!(lim.check("b").is_ok(), "buckets are per key");
    t.fetch_add(1000, Ordering::SeqCst);
    assert!(lim.check("a").is_ok());
    assert!(lim.check("a").is_err());
    t.fetch_add(3_600_000, Ordering::SeqCst);
    assert!(lim.check("a").is_ok() && lim.check("a").is_ok() && lim.check("a").is_err());
}

#[test]
fn many_keys_do_not_grow_the_table_without_end() {
    let (_, clock) = manual_clock();
    let lim = RateLimiter::new(
        Rate {
            burst: 1,
            per_second: 1,
        },
        clock,
    );
    for i in 0..50_000 {
        let _ = lim.check(&format!("ip{i}"));
    }
    assert!(lim.len() <= 10_000);
}

/// A body that records whether anyone read it.
fn watched(polled: Arc<AtomicBool>) -> Body {
    let s = futures_util::stream::poll_fn(move |_| {
        polled.store(true, Ordering::SeqCst);
        std::task::Poll::Ready(None::<Result<Vec<u8>, std::io::Error>>)
    });
    Body::from_stream(s)
}

#[tokio::test]
async fn a_limited_client_is_refused_before_its_body_is_read() {
    let (_, clock) = manual_clock();
    let mut c = config();
    c.per_client = Rate {
        burst: 1,
        per_second: 1,
    };
    c.clock = clock;
    let fake = Fake::ok();
    let router = app(fake.clone(), c);
    let first = router
        .clone()
        .oneshot(from(
            "10.0.0.1",
            post("paint", Some(TOKEN), body(vec![add("k", "a")])),
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), 200);

    let polled = Arc::new(AtomicBool::new(false));
    let mut req = Request::post("/v1/files/f/tools/paint")
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(watched(polled.clone()))
        .unwrap();
    req = from("10.0.0.1", req);
    let (status, v) = json(router.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (429, Some("rate_limited"))
    );
    assert!(!polled.load(Ordering::SeqCst), "the body was not read");

    // Garbage and oversized bodies from the limited client are 429, not 400 or 413: never parsed or measured.
    for payload in [b"not json".to_vec(), vec![b'x'; 5_000_000]] {
        let res = router
            .clone()
            .oneshot(from("10.0.0.1", post("paint", Some(TOKEN), payload)))
            .await
            .unwrap();
        assert_eq!(res.status(), 429);
        assert!(res.headers().get("retry-after").is_some());
    }
    // Another client is not limited by it.
    let other = router
        .oneshot(from(
            "10.0.0.2",
            post("paint", Some(TOKEN), body(vec![add("k", "a")])),
        ))
        .await
        .unwrap();
    assert_eq!(other.status(), 200);
    assert_eq!(fake.calls(), 2);
}

#[tokio::test]
async fn unauthenticated_requests_still_spend_the_client_budget() {
    let (_, clock) = manual_clock();
    let mut c = config();
    c.per_client = Rate {
        burst: 2,
        per_second: 1,
    };
    c.clock = clock;
    let router = app(Fake::ok(), c);
    let mut seen = Vec::new();
    for _ in 0..3 {
        let res = router
            .clone()
            .oneshot(from("10.0.0.9", post("paint", Some("guess"), body(vec![]))))
            .await
            .unwrap();
        seen.push(res.status().as_u16());
    }
    assert_eq!(seen, [401, 401, 429]);
}

#[tokio::test]
async fn a_principal_is_limited_across_clients() {
    let (_, clock) = manual_clock();
    let mut c = config();
    c.per_principal = Rate {
        burst: 1,
        per_second: 1,
    };
    c.clock = clock;
    let router = app(Fake::ok(), c);
    let one = router
        .clone()
        .oneshot(from(
            "10.0.0.1",
            post("paint", Some(TOKEN), body(vec![add("k", "a")])),
        ))
        .await
        .unwrap();
    let two = router
        .clone()
        .oneshot(from(
            "10.0.0.2",
            post("paint", Some(TOKEN), body(vec![add("k", "a")])),
        ))
        .await
        .unwrap();
    let third = router
        .oneshot(from(
            "10.0.0.3",
            post("paint", Some(OTHER), body(vec![add("k", "a")])),
        ))
        .await
        .unwrap();
    assert_eq!(
        (
            one.status().as_u16(),
            two.status().as_u16(),
            third.status().as_u16()
        ),
        (200, 429, 200)
    );
}
