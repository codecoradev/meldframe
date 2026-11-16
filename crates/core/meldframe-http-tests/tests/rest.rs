//! REST: `POST /v1/files/{file}/tools/{tool}`. Every case goes through the router, as a client would.

mod common;
use axum::{body::Body, http::Request};
use common::*;
use meldframe_host::Refusal;
use meldframe_http::Failure;
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_server::{Server, SharedServer};
use std::sync::Arc;
use std::time::Duration;
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

fn real() -> Arc<SharedServer> {
    Arc::new(SharedServer::new(Server::assemble_with(vec![Box::new(
        Painter,
    )])))
}

#[tokio::test]
async fn no_token_and_a_wrong_token_are_unauthenticated_and_reach_nothing() {
    let fake = Fake::ok();
    for token in [None, Some("nope")] {
        let res = app(fake.clone(), config())
            .oneshot(post("paint", token, body(vec![add("k", "a")])))
            .await
            .unwrap();
        let (status, v) = json(res).await;
        assert_eq!(
            (status, v["error"]["code"].as_str()),
            (401, Some("unauthenticated"))
        );
    }
    assert_eq!(fake.calls(), 0);
}

#[tokio::test]
async fn a_dev_token_submits_and_the_op_is_in_the_room_of_its_workspace() {
    let server = real();
    let res = app(server.clone(), config())
        .oneshot(post("paint", Some(TOKEN), body(vec![add("k1", "a1")])))
        .await
        .unwrap();
    let (status, v) = json(res).await;
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["accepted"][0]["seq"], 1);
    assert_eq!(v["accepted"][0]["written"], true);
    let ids = server.with_server(|s| s.with_room("ws_1", "file_a", |r| r.nodes().len()));
    assert_eq!(ids, Some(1));
    let elsewhere = server.with_server(|s| s.with_room("ws_2", "file_a", |r| r.nodes().len()));
    assert_eq!(
        elsewhere, None,
        "the principal's workspace comes from the token"
    );
}

#[tokio::test]
async fn sending_the_same_key_again_is_a_duplicate() {
    let server = real();
    for written in [true, false] {
        let res = app(server.clone(), config())
            .oneshot(post("paint", Some(TOKEN), body(vec![add("k1", "a1")])))
            .await
            .unwrap();
        let (_, v) = json(res).await;
        assert_eq!(v["accepted"][0]["written"], written);
    }
}

#[tokio::test]
async fn refusals_of_the_gate_keep_their_stable_codes_and_get_a_status() {
    let server = real();
    let cases = [
        ("no_such_tool", vec![add("k", "a")], 403, "forbidden"),
        ("paint", vec![], 400, "invalid"),
    ];
    for (tool, ops, status, code) in cases {
        let res = app(server.clone(), config())
            .oneshot(post(tool, Some(TOKEN), body(ops)))
            .await
            .unwrap();
        let (got, v) = json(res).await;
        assert_eq!(
            (got, v["error"]["code"].as_str()),
            (status, Some(code)),
            "{tool}"
        );
    }
}

#[tokio::test]
async fn every_refusal_and_failure_maps_to_one_code_and_status() {
    use meldframe_check::ApplyError;
    use meldframe_module::Policy;
    let cases: Vec<(Failure, u16, &str)> = vec![
        (
            Refusal::Forbidden {
                tier: RiskTier::Delete,
                policy: Policy::Ask,
            }
            .into(),
            403,
            "forbidden",
        ),
        (
            Refusal::QuotaExceeded { limit: 1 }.into(),
            429,
            "quota_exceeded",
        ),
        (
            Refusal::Conflict {
                node_id: "n".into(),
            }
            .into(),
            409,
            "conflict",
        ),
        (
            Refusal::Invalid { reason: "r".into() }.into(),
            400,
            "invalid",
        ),
        (
            Refusal::DependencyRejected { key: "k".into() }.into(),
            422,
            "dependency_rejected",
        ),
        (
            Refusal::Apply(ApplyError::NotFound("n".into())).into(),
            422,
            "E_NODE_NOT_FOUND",
        ),
        (
            Failure::Unavailable("store down".into()),
            503,
            "unavailable",
        ),
    ];
    for (failure, status, code) in cases {
        let res = app(Fake::failing(failure), config())
            .oneshot(post("paint", Some(TOKEN), body(vec![add("k", "a")])))
            .await
            .unwrap();
        let (got, v) = json(res).await;
        assert_eq!((got, v["error"]["code"].as_str()), (status, Some(code)));
    }
}

#[tokio::test]
async fn malformed_bodies_get_a_stable_error_and_the_server_keeps_working() {
    let fake = Fake::ok();
    let bad: Vec<Vec<u8>> = vec![
        b"not json".to_vec(),
        b"{\"ops\": 5}".to_vec(),
        b"{\"ops\": [], \"surprise\": 1}".to_vec(),
        b"{\"ops\": [{\"key\": \"k\", \"command\": {\"type\": \"explode\"}}]}".to_vec(),
        vec![0xff, 0xfe, 0x00],
        Vec::new(),
    ];
    for payload in bad {
        let res = app(fake.clone(), config())
            .oneshot(post("paint", Some(TOKEN), payload))
            .await
            .unwrap();
        let (status, v) = json(res).await;
        assert_eq!(
            (status, v["error"]["code"].as_str()),
            (400, Some("bad_request"))
        );
    }
    assert_eq!(fake.calls(), 0);
    let router = app(fake.clone(), config());
    let ok = router
        .oneshot(post("paint", Some(TOKEN), body(vec![add("k", "a")])))
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);
}

#[tokio::test]
async fn a_body_over_the_limit_is_refused_as_too_large() {
    let mut c = config();
    c.max_body = 64;
    let fake = Fake::ok();
    let res = app(fake.clone(), c)
        .oneshot(post("paint", Some(TOKEN), vec![b' '; 4096]))
        .await
        .unwrap();
    let (status, v) = json(res).await;
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (413, Some("too_large"))
    );
    assert_eq!(fake.calls(), 0);
}

#[tokio::test]
async fn a_call_over_its_time_limit_is_a_timeout_and_the_next_call_works() {
    let mut c = config();
    c.timeout = Duration::from_millis(50);
    let slow = Fake::slow(Duration::from_millis(400));
    let router = app(slow.clone(), c);
    let res = router
        .clone()
        .oneshot(post("paint", Some(TOKEN), body(vec![add("k", "a")])))
        .await
        .unwrap();
    let (status, v) = json(res).await;
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (504, Some("timeout"))
    );
    tokio::time::sleep(Duration::from_millis(500)).await;
    slow.set_delay(Duration::ZERO);
    let res = router
        .oneshot(post("paint", Some(TOKEN), body(vec![add("k", "a")])))
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
}

#[tokio::test]
async fn a_panic_in_the_store_is_an_internal_error_not_a_dead_server() {
    let fake = Fake::broken();
    let router = app(fake.clone(), config());
    let first = router
        .clone()
        .oneshot(post("paint", Some(TOKEN), body(vec![add("k", "a")])))
        .await
        .unwrap();
    let (status, v) = json(first).await;
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (500, Some("internal"))
    );
    let second = router
        .oneshot(post("paint", Some(TOKEN), body(vec![add("k", "a")])))
        .await
        .unwrap();
    assert_eq!(second.status(), 200);
}

#[tokio::test]
async fn the_only_write_route_is_the_tool_route() {
    let fake = Fake::ok();
    for (method, path) in [
        ("POST", "/v1/files/file_a/ops"),
        ("POST", "/v1/files/file_a"),
        ("POST", "/v1/rooms/file_a/submit"),
        ("PUT", "/v1/files/file_a/tools/paint"),
        ("GET", "/v1/files/file_a/tools/paint"),
    ] {
        let req = Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", format!("Bearer {TOKEN}"))
            .body(Body::from(body(vec![add("k", "a")])))
            .unwrap();
        let res = app(fake.clone(), config()).oneshot(req).await.unwrap();
        assert!(
            matches!(res.status().as_u16(), 404 | 405),
            "{method} {path}"
        );
    }
    assert_eq!(fake.calls(), 0);
}
