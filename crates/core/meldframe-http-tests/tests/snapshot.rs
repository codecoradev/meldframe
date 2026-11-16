//! `GET /v1/files/{file}`: the file as it is now (nodes and sequence number), for a client that starts
//! from a snapshot and then follows the live stream. The principal and its workspace come from the token.

mod common;
use axum::{Router, body::Body, http::Request};
use common::*;
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_server::{Server, SharedServer};
use std::sync::Arc;
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

fn serving(server: &Arc<SharedServer>) -> Router {
    let state = meldframe_http::shared(tokens(), server.clone(), config());
    meldframe_http::routes(state.clone())
        .merge(meldframe_http_snapshot::routes(state, server.clone()))
}

fn get(file: &str, token: Option<&str>) -> Request<Body> {
    let mut r = Request::get(format!("/v1/files/{file}"));
    if let Some(t) = token {
        r = r.header("authorization", format!("Bearer {t}"));
    }
    r.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn without_a_valid_token_nothing_is_read() {
    let server = real();
    for token in [None, Some("nope")] {
        let res = serving(&server)
            .oneshot(get("file_a", token))
            .await
            .unwrap();
        let (status, v) = json(res).await;
        assert_eq!(
            (status, v["error"]["code"].as_str()),
            (401, Some("unauthenticated"))
        );
    }
}

#[tokio::test]
async fn a_new_file_reads_as_sequence_zero_and_empty() {
    let server = real();
    let res = serving(&server)
        .oneshot(get("file_a", Some(TOKEN)))
        .await
        .unwrap();
    let (status, v) = json(res).await;
    assert_eq!(status, 200, "{v}");
    assert_eq!(v, serde_json::json!({"seq": 0, "nodes": []}));
}

#[tokio::test]
async fn it_returns_the_nodes_and_the_sequence_number_of_the_principals_workspace() {
    let server = real();
    let sent = app(server.clone(), config())
        .oneshot(post("paint", Some(TOKEN), body(vec![add("k1", "a1")])))
        .await
        .unwrap();
    assert_eq!(json(sent).await.0, 200);

    let (status, v) = json(
        serving(&server)
            .oneshot(get("file_a", Some(TOKEN)))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(v["seq"], 1);
    assert_eq!(v["nodes"][0]["id"], "a1");
    assert_eq!(v["nodes"][0]["type"], "frame");

    // Another workspace has its own file of the same name.
    let (_, other) = json(
        serving(&server)
            .oneshot(get("file_a", Some(OTHER)))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(other, serde_json::json!({"seq": 0, "nodes": []}));
}

#[tokio::test]
async fn a_bad_file_name_is_invalid() {
    let server = real();
    let res = serving(&server)
        .oneshot(get("%2e%2e", Some(TOKEN)))
        .await
        .unwrap();
    let (status, v) = json(res).await;
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (400, Some("invalid"))
    );
}
