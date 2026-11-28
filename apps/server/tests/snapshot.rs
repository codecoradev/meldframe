//! `GET /v1/files/{file}`: the file as it is now (sequence number and nodes), each node with the actor and the
//! sequence number of the last op that touched it, for a client that starts from a snapshot and then follows
//! the live stream. The principal and its workspace come from the token. The log carries no time, so there
//! is no `lastAt`.

use axum::{Router, body::Body, http::Request};
use meldframe_host::{Op, Principal};
use meldframe_http::{Authenticator, Config, DevTokens, Rate};
use meldframe_http_snapshot::snapshot_of;
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::{Command, NodePatch};
use meldframe_server::{Server, SharedServer};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

const ALICE: &str = "tok-user-alice-001";
const BOB: &str = "tok-user-bob-00002";
const CAROL: &str = "tok-user-carol-003";

struct Painter;
impl Module for Painter {
    fn name(&self) -> &'static str {
        "painter"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
    }
}

fn tokens() -> Arc<dyn Authenticator> {
    Arc::new(
        DevTokens::parse(&format!(
            "{ALICE}:ws_1:user:alice,{BOB}:ws_1:user:bob,{CAROL}:ws_2:user:carol"
        ))
        .unwrap(),
    )
}

fn config() -> Config {
    let rate = Rate {
        burst: 1000,
        per_second: 1000,
    };
    Config {
        per_client: rate,
        per_principal: rate,
        ..Config::default()
    }
}

async fn json(res: axum::response::Response) -> (u16, Value) {
    let status = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

fn who(actor: &str) -> Principal {
    Principal::new("ws_1", actor)
}

fn server() -> Server {
    Server::assemble_with(vec![Box::new(Painter)])
}

fn router(server: &Arc<SharedServer>) -> Router {
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

async fn read(server: &Arc<SharedServer>, file: &str, token: &str) -> (u16, Value) {
    let res = router(server)
        .oneshot(get(file, Some(token)))
        .await
        .unwrap();
    json(res).await
}

fn add(key: &str, id: &str) -> Op {
    let node = serde_json::from_value(json!({"id": id, "type": "rect"})).unwrap();
    Op::new(key, Command::AddNode { node, index: None })
}

fn nudge(key: &str, id: &str, x: f64) -> Op {
    let patch = NodePatch {
        x: Some(x),
        ..NodePatch::default()
    };
    Op::new(
        key,
        Command::UpdateNode {
            id: id.into(),
            patch,
        },
    )
}

fn remove(key: &str, id: &str) -> Op {
    Op::new(key, Command::RemoveNode { id: id.into() })
}

fn reorder_after(key: &str, id: &str, after: &str) -> Op {
    Op::new(
        key,
        Command::Reorder {
            id: id.into(),
            before: None,
            after: Some(after.into()),
            pos: None,
        },
    )
}

fn put(s: &mut Server, actor: &str, ops: Vec<Op>) {
    s.submit_tool("paint", "file_a", &who(actor), ops).unwrap();
}

/// `(lastActor, lastSeq)` of node `id` in a snapshot body.
fn last(v: &Value, id: &str) -> (Option<String>, Option<u64>) {
    let n = v["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == id)
        .unwrap_or_else(|| panic!("node {id} is not in {v}"));
    (
        n.get("lastActor").map(|a| a.as_str().unwrap().to_owned()),
        n.get("lastSeq").map(|s| s.as_u64().unwrap()),
    )
}

#[tokio::test]
async fn without_a_valid_token_nothing_is_read() {
    let server = Arc::new(SharedServer::new(server()));
    for token in [None, Some("nope")] {
        let res = router(&server).oneshot(get("file_a", token)).await.unwrap();
        let (status, v) = json(res).await;
        assert_eq!(
            (status, v["error"]["code"].as_str()),
            (401, Some("unauthenticated"))
        );
    }
}

#[tokio::test]
async fn a_bad_file_name_is_invalid_and_an_unknown_file_is_not_found() {
    let server = Arc::new(SharedServer::new(server()));
    let (status, v) = read(&server, "%2e%2e", ALICE).await;
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (400, Some("invalid"))
    );
    let (status, v) = read(&server, "file_a", ALICE).await;
    assert_eq!(
        (
            status,
            v["error"]["code"].as_str(),
            v["error"]["retryable"].as_bool()
        ),
        (404, Some("not_found"), Some(false))
    );
    // Reading wrote nothing: the file still does not exist.
    assert_eq!(read(&server, "file_a", ALICE).await.0, 404);
}

#[tokio::test]
async fn it_returns_the_file_of_the_principals_workspace_only() {
    let mut s = server();
    put(&mut s, "user:alice", vec![add("k1", "a1")]);
    let server = Arc::new(SharedServer::new(s));
    let (status, v) = read(&server, "file_a", BOB).await;
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["seq"], 1);
    assert_eq!(v["nodes"][0]["id"], "a1");
    assert_eq!(v["nodes"][0]["type"], "rect");
    // Another workspace has no such file: not found, the same answer as for a name nobody used.
    let (status, v) = read(&server, "file_a", CAROL).await;
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (404, Some("not_found"))
    );
}

#[tokio::test]
async fn a_node_carries_the_actor_and_the_sequence_number_of_its_last_op() {
    let mut s = server();
    put(&mut s, "user:alice", vec![add("k1", "n1"), add("k2", "n2")]); // seq 1, 2
    put(&mut s, "user:bob", vec![nudge("k1", "n1", 10.0)]); // seq 3
    let server = Arc::new(SharedServer::new(s));
    let (_, v) = read(&server, "file_a", ALICE).await;
    assert_eq!(v["seq"], 3);
    assert_eq!(last(&v, "n1"), (Some("user:bob".into()), Some(3)));
    assert_eq!(last(&v, "n2"), (Some("user:alice".into()), Some(2)));
    for n in v["nodes"].as_array().unwrap() {
        assert!(n.get("lastAt").is_none(), "the log has no time: {n}");
    }
}

#[tokio::test]
async fn an_undo_by_a_makes_a_the_last_actor_and_a_removed_node_is_gone() {
    let mut s = server();
    put(&mut s, "user:alice", vec![add("k1", "n1"), add("k2", "n2")]); // 1, 2
    put(&mut s, "user:bob", vec![nudge("k1", "n1", 10.0)]); // 3
    // An undo is an ordinary op of the actor who undoes: here alice puts the old value back.
    put(&mut s, "user:alice", vec![nudge("k3", "n1", 0.0)]); // 4
    put(&mut s, "user:bob", vec![remove("k2", "n2")]); // 5
    let server = Arc::new(SharedServer::new(s));
    let (_, v) = read(&server, "file_a", BOB).await;
    assert_eq!(last(&v, "n1"), (Some("user:alice".into()), Some(4)));
    assert_eq!(
        v["nodes"].as_array().unwrap().len(),
        1,
        "n2 is removed: {v}"
    );
}

#[tokio::test]
async fn a_rebalance_by_bob_makes_bob_the_last_actor_of_the_siblings_it_reordered() {
    let mut s = server();
    let adds = vec![add("k1", "n1"), add("k2", "n2"), add("k3", "n3")];
    put(&mut s, "user:alice", adds); // 1, 2, 3
    // A rebalance is plain reorders (`rebalance_commands`), written by whoever asked for it.
    put(
        &mut s,
        "user:bob",
        vec![
            reorder_after("r1", "n1", "n3"),
            reorder_after("r2", "n2", "n1"),
        ], // 4, 5
    );
    let server = Arc::new(SharedServer::new(s));
    let (_, v) = read(&server, "file_a", ALICE).await;
    assert_eq!(last(&v, "n1"), (Some("user:bob".into()), Some(4)));
    assert_eq!(last(&v, "n2"), (Some("user:bob".into()), Some(5)));
    assert_eq!(last(&v, "n3"), (Some("user:alice".into()), Some(3)));
}

/// What a room says now, and what a room restored from the store says: the same, also across the room's
/// periodic snapshot (every 500 ops), because the answer comes from the log and not from a map kept in memory.
#[test]
fn a_file_restored_from_the_store_answers_as_the_live_room_did() {
    let mut s = server();
    let (alice, bob) = (who("user:alice"), who("user:bob"));
    for n in 0..4 {
        put(
            &mut s,
            "user:alice",
            vec![add(&format!("a{n}"), &format!("n{n}"))],
        );
    }
    // 520 more ops, so the room saves a snapshot at 500 and a restore replays only the tail.
    for i in 0..520u32 {
        let (actor, id) = if i % 3 == 0 {
            (&bob, "n1")
        } else {
            (&alice, "n2")
        };
        let op = nudge(&format!("u{i}"), id, f64::from(i));
        s.submit_tool("paint", "file_a", actor, vec![op]).unwrap();
    }
    let now = |s: &Server| {
        let snap = s
            .with_room("ws_1", "file_a", |r| snapshot_of(r, 10_000))
            .unwrap()
            .unwrap()
            .unwrap();
        serde_json::to_value(snap).unwrap()
    };
    let live = now(&s);
    assert_eq!(live["seq"], 524);
    assert_eq!(last(&live, "n0"), (Some("user:alice".into()), Some(1)));
    assert_eq!(last(&live, "n3"), (Some("user:alice".into()), Some(4)));
    s.close("ws_1", "file_a");
    s.open("ws_1", "file_a").unwrap();
    assert_eq!(now(&s), live);
}

#[test]
fn a_node_older_than_the_scanned_window_has_no_last_actor_and_the_rest_are_exact() {
    let mut s = server();
    put(&mut s, "user:alice", vec![add("k1", "old")]); // 1
    put(
        &mut s,
        "user:bob",
        vec![add("k2", "n2"), add("k3", "n3"), add("k4", "n4")],
    ); // 2..4
    let snap = s
        .with_room("ws_1", "file_a", |r| snapshot_of(r, 2))
        .unwrap()
        .unwrap()
        .unwrap();
    let v = serde_json::to_value(snap).unwrap();
    assert_eq!(last(&v, "old"), (None, None));
    assert_eq!(
        last(&v, "n2"),
        (None, None),
        "seq 2 is outside the last two ops"
    );
    assert_eq!(last(&v, "n3"), (Some("user:bob".into()), Some(3)));
    assert_eq!(last(&v, "n4"), (Some("user:bob".into()), Some(4)));
}
