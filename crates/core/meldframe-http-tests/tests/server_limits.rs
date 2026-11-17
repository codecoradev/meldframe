//! How the server's own limits (names, batch size, room caps) reach a client: the transport code each one
//! gets, REST status included, and the `Server limits` table of `docs/error-codes.md` held against it.

mod common;

use axum::body::Body;
use axum::http::Request;
use axum::response::IntoResponse;
use common::{add, body, config, json};
use meldframe_host::{Op, Principal, Refusal};
use meldframe_http::mcp::Reader;
use meldframe_http::{Authenticator, Failure, Reply, Submitter, router};
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::Command;
use meldframe_server::{Server, SharedServer};
use std::sync::Arc;
use tower::ServiceExt;

struct Tools;
impl Module for Tools {
    fn name(&self) -> &'static str {
        "tools"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
        r.agent_tool("peek", "Look at a node.", RiskTier::Read);
    }
}

fn shared(per_workspace: usize) -> Arc<SharedServer> {
    let s = Server::assemble_with(vec![Box::new(Tools)]).with_workspace_rooms(per_workspace);
    Arc::new(SharedServer::new(s))
}

fn who(ws: &str) -> Principal {
    Principal::new(ws, "agent:a")
}

fn op(n: usize) -> Op {
    let node = serde_json::from_value(serde_json::json!({"id": format!("n{n}"), "type": "frame"}))
        .unwrap();
    Op::new(format!("k{n}"), Command::AddNode { node, index: None })
}

fn code(f: &Failure) -> (u16, String, bool) {
    let reply = Reply::failure(f).into_response();
    let status = reply.status().as_u16();
    let bytes =
        futures_util::FutureExt::now_or_never(axum::body::to_bytes(reply.into_body(), 1 << 16))
            .unwrap()
            .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    (
        status,
        v["error"]["code"].as_str().unwrap().to_string(),
        v["error"]["retryable"].as_bool().unwrap(),
    )
}

fn bad_names() -> Vec<String> {
    vec![
        String::new(),
        "x".repeat(129),
        "a\0b".into(),
        "a\u{202E}b".into(),
    ]
}

struct Fixed(Principal);
impl Authenticator for Fixed {
    fn authenticate(&self, _: &str) -> Option<Principal> {
        Some(self.0.clone())
    }
}

#[tokio::test]
async fn a_bad_workspace_name_is_invalid_400_over_rest_and_never_retryable() {
    for ws in bad_names() {
        let app = router(Arc::new(Fixed(who(&ws))), shared(4), config());
        let req = Request::post("/v1/files/file_a/tools/paint")
            .header("content-type", "application/json")
            .header("authorization", "Bearer any-token-1234567")
            .body(Body::from(body(vec![add("k", "a")])))
            .unwrap();
        let (status, v) = json(app.oneshot(req).await.unwrap()).await;
        assert_eq!(
            (
                status,
                v["error"]["code"].as_str(),
                v["error"]["retryable"].as_bool()
            ),
            (400, Some("invalid"), Some(false)),
            "workspace {ws:?}"
        );
    }
}

#[test]
fn bad_names_are_invalid_on_every_server_path() {
    let s = shared(4);
    for name in bad_names() {
        for (ws, file) in [(name.as_str(), "f"), ("ws_1", name.as_str())] {
            let who = who(ws);
            let all = [
                s.submit("paint", file, &who, vec![op(1)]).unwrap_err(),
                s.submit_atomic("paint", file, &who, vec![op(1)], vec![])
                    .unwrap_err(),
                s.view(&who, file).unwrap_err(),
            ];
            for f in all {
                assert_eq!(code(&f), (400, "invalid".into(), false), "{ws:?} {file:?}");
            }
        }
    }
}

#[test]
fn submit_atomic_runs_the_cheap_checks_before_it_opens_a_room() {
    let s = shared(1);
    s.submit("paint", "held", &who("ws_1"), vec![op(1)])
        .unwrap();
    // At its room limit, an open of "other" would fail as unavailable; these must be refused first.
    let many: Vec<Op> = (0..101).map(op).collect();
    let cases = [
        ("paint", many, "batch_too_large"),
        ("peek", vec![op(1)], "forbidden"),
        ("nope", vec![op(1)], "forbidden"),
    ];
    for (tool, ops, want) in cases {
        let err = s
            .submit_atomic(tool, "other", &who("ws_1"), ops, vec![])
            .unwrap_err();
        assert_eq!(code(&err).1, want, "{tool}");
    }
}

#[test]
fn the_server_limits_table_matches_the_transport_codes() {
    let docs = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/error-codes.md"
    ))
    .unwrap();
    let section = docs
        .split("\n## Server limits")
        .nth(1)
        .expect("a `## Server limits` section")
        .split("\n## ")
        .next()
        .unwrap();
    let rows: Vec<Vec<&str>> = section
        .lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| Check") && !l.starts_with("| ---"))
        .map(|l| l.split(" | ").collect())
        .collect();
    let s = shared(1);
    s.submit("paint", "held", &who("ws_1"), vec![op(1)])
        .unwrap();
    let many: Vec<Op> = (0..101).map(op).collect();
    let mut seen = Vec::new();
    for row in rows {
        let typed = row[1].trim_matches(['`', ' ']).to_string();
        let want = row[2].trim_matches(['`', ' ', '|']).to_string();
        let got = match typed.as_str() {
            "RegistryError::InvalidName" => s.submit("paint", "a\0", &who("ws_1"), vec![op(1)]),
            "Refusal::BatchTooLarge" => s.submit("paint", "other", &who("ws_1"), many.clone()),
            "Refusal::Forbidden" => s.submit("peek", "other", &who("ws_1"), vec![op(1)]),
            "RegistryError::WorkspaceFull" => s.submit("paint", "other", &who("ws_1"), vec![op(2)]),
            "RegistryError::Full" => {
                // The global cap maps like the workspace cap: any non-refusal registry error.
                s.submit("paint", "other", &who("ws_1"), vec![op(2)])
            }
            other => panic!("the table has a case the test does not know: {other}"),
        };
        assert_eq!(code(&got.unwrap_err()).1, want, "{typed}");
        seen.push(typed);
    }
    assert_eq!(seen.len(), 5, "{seen:?}");
}

#[test]
fn a_refusal_is_a_refusal() {
    let s = shared(1);
    let err = s
        .submit("peek", "f", &who("ws_1"), vec![op(1)])
        .unwrap_err();
    assert!(matches!(err, Failure::Refused(Refusal::Forbidden { .. })));
}
