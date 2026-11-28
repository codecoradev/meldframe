//! `GET /v1/files/{file}/checkpoints/{n}/state` through the real router (ADR-0023, slice 6): the file as it was at a
//! checkpoint, worked out by `state_at` from the latest snapshot at or before it (or from 0), the bound, the cost, the
//! cache with its byte bound, and who may ask.

mod world;
use checkpoint_store::CheckpointStore;
use meldframe_checkpoints::{Kind, NewCheckpoint};
use meldframe_host::{NewOp, Principal, Snapshot, Store, StoredOp};
use meldframe_http::Rate;
use meldframe_http::mcp::Reader;
use meldframe_module::Policy;
use meldframe_protocol::Command;
use meldframe_revert::{LoggedOp, state_at};
use meldframe_server::{Server, SharedServer};
use meldframe_store_pg::{Backing, room_key};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use world::*;

const BEA: &str = PERSON;
const WS: &str = "ws_1";

fn op(key: &str, command: Value) -> Value {
    json!({"key": key, "command": command})
}

fn rect(key: &str, id: &str, parent: &str) -> Value {
    let node = json!({"id": id, "type": "rect", "name": id, "parentId": parent, "w": 10, "h": 10});
    op(key, json!({"type": "addNode", "node": node}))
}

fn patch(key: &str, id: &str, x: f64) -> Value {
    op(
        key,
        json!({"type": "updateNode", "id": id, "patch": {"x": x}}),
    )
}

fn reparent(key: &str, id: &str, parent: &str) -> Value {
    op(
        key,
        json!({"type": "reparent", "id": id, "parentId": parent}),
    )
}

fn remove(key: &str, id: &str) -> Value {
    op(key, json!({"type": "removeNode", "id": id}))
}

/// A history of three actors, one op each step: eight ops, the last one a removal.
async fn script(w: &World) {
    let steps: [(&str, Value); 8] = [
        (AGENT, add("a1", "fa")),
        (PERSON, add("p1", "fb")),
        (DELTA, rect("d1", "r1", "fa")),
        (AGENT, patch("a2", "fa", 40.0)),
        (PERSON, patch("p2", "r1", 7.0)),
        (DELTA, reparent("d2", "r1", "fb")),
        (PERSON, remove("p3", "r1")),
        (AGENT, patch("a3", "fb", 90.0)),
    ];
    for (token, step) in steps {
        let (status, body) = rest(w, token, "f", vec![step]).await;
        assert_eq!(status, 200, "{body}");
    }
}

fn logged(ops: Vec<StoredOp>) -> Vec<LoggedOp> {
    let one = |o: StoredOp| LoggedOp {
        seq: o.seq,
        actor: o.actor,
        key: o.key,
        command: o.command,
    };
    ops.into_iter().map(one).collect()
}

/// The log of `file` as the app's record would hold it.
fn log_of(w: &World, file: &str) -> Vec<LoggedOp> {
    let reader = Principal::new(WS, "user:bea");
    logged(w.server.tree(&reader, file, 100).unwrap().1)
}

/// A snapshot of `file` at `seq`, the state of the first `seq` ops of `log`, kept in the backing store.
fn keep_snapshot(backing: &Backing, file: &str, log: &[LoggedOp], seq: u64) {
    let nodes = state_at(&[], log, seq).unwrap();
    let variables = Default::default();
    let snapshot = Snapshot {
        seq,
        nodes,
        variables,
    };
    let saved = backing.store().save_snapshot(&room_key(WS, file), snapshot);
    saved.unwrap();
}

/// A manual row of `file` at `seq`, made through the store (the routes of the create are another file's tests).
fn row(w: &World, ws: &str, file: &str, seq: u64) -> u32 {
    let new = NewCheckpoint::by_person(Kind::Manual, seq, None, None, "user:bea").unwrap();
    w.store
        .create(ws, file, None, &new, 1)
        .unwrap()
        .checkpoint
        .n
}

async fn state(w: &World, token: &str, file: &str, n: impl std::fmt::Display) -> (u16, Value) {
    get(
        w,
        Some(token),
        &format!("/v1/files/{file}/checkpoints/{n}/state"),
    )
    .await
}

/// A fresh server and a world on it with `spec`.
async fn with(spec: Spec) -> World {
    start_on(Arc::new(SharedServer::new(Server::assemble())), spec).await
}

fn opened(w: &World) -> usize {
    w.past.opened.load(Ordering::SeqCst)
}

fn snapshots(w: &World) -> usize {
    w.past.snapshots.load(Ordering::SeqCst)
}

fn code(r: &(u16, Value)) -> &str {
    r.1["error"]["code"].as_str().unwrap_or("")
}

fn expected(log: &[LoggedOp], seq: u64) -> Value {
    let nodes = state_at(&[], log, seq).unwrap();
    json!({"seq": seq, "nodes": nodes})
}

#[tokio::test]
async fn the_answer_is_the_state_at_for_every_seq_of_a_history_of_three_actors() {
    let w = start().await;
    script(&w).await;
    let log = log_of(&w, "f");
    assert_eq!(log.len(), 8);
    for seq in 0..=8 {
        let n = row(&w, WS, "f", seq);
        let (status, body) = state(&w, BEA, "f", n).await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body, expected(&log, seq), "at seq {seq}");
    }
    let (_, at_six) = state(&w, BEA, "f", row(&w, WS, "f", 6)).await;
    let (_, at_seven) = state(&w, BEA, "f", row(&w, WS, "f", 7)).await;
    assert_ne!(at_six["nodes"], at_seven["nodes"]);
}

#[tokio::test]
async fn the_replay_starts_at_the_latest_snapshot_at_or_before_the_checkpoint_and_else_at_zero() {
    let backing = Backing::memory();
    let server = Arc::new(SharedServer::new(Server::assemble_in(
        Vec::new(),
        backing.clone(),
    )));
    let w = start_on(server, Spec::default()).await;
    script(&w).await;
    let log = log_of(&w, "f");
    keep_snapshot(&backing, "f", &log, 4);
    // After the snapshot: from 4. At it: no op is read. Before it: from 0.
    for (seq, from) in [
        (8, Some(4)),
        (6, Some(4)),
        (4, None),
        (3, Some(0)),
        (1, Some(0)),
    ] {
        w.past.reset();
        let (status, body) = state(&w, BEA, "f", row(&w, WS, "f", seq)).await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body, expected(&log, seq), "at seq {seq}");
        let after = w.past.afters.lock().unwrap().first().copied();
        assert_eq!(after, from, "the first page of the replay for seq {seq}");
        assert_eq!(opened(&w), 1);
    }
}

/// `n` ops written to `file` of `ws_1` straight into the store: one frame and `n - 1` moves of it.
fn long_log(backing: &Backing, file: &str, n: u64) {
    let mut store = backing.store();
    let add = json!({"type": "addNode", "node": {"id": "f1", "type": "frame", "name": "f1"}});
    let moves = (1..n).map(|i| json!({"type": "updateNode", "id": "f1", "patch": {"x": i as f64}}));
    for (i, command) in std::iter::once(add).chain(moves).enumerate() {
        let command: Command = serde_json::from_value(command).unwrap();
        let op = NewOp {
            actor: "agent:alpha".into(),
            key: format!("k{i}"),
            command,
        };
        store.append(&room_key(WS, file), op).unwrap();
    }
}

async fn long_world(snapshot_at: Option<u64>) -> World {
    let backing = Backing::memory();
    long_log(&backing, "big", 5_001);
    if let Some(seq) = snapshot_at {
        let ops = backing.store().ops_after(&room_key(WS, "big"), 0, 5_001);
        keep_snapshot(&backing, "big", &logged(ops.unwrap()), seq);
    }
    let server = Arc::new(SharedServer::new(Server::assemble_in(Vec::new(), backing)));
    let timeout = Duration::from_secs(60);
    start_on(
        server,
        Spec {
            timeout,
            ..Spec::default()
        },
    )
    .await
}

#[tokio::test]
async fn a_replay_past_five_thousand_ops_is_too_far_and_reads_no_op() {
    let w = long_world(None).await;
    let past = row(&w, WS, "big", 5_001);
    let at = row(&w, WS, "big", 5_000);
    let r = state(&w, BEA, "big", past).await;
    assert_eq!((r.0, code(&r)), (422, "too_far"), "{}", r.1);
    assert_eq!(r.1["error"]["retryable"], false);
    assert_eq!(w.past.pages(), 0, "no op was paged");
    let r = state(&w, BEA, "big", at).await;
    assert_eq!(r.0, 200, "{}", r.1);
    assert_eq!(r.1["seq"], 5_000);
    assert_eq!(r.1["nodes"][0]["x"], 4_999.0);
}

#[tokio::test]
async fn a_snapshot_brings_an_old_file_inside_the_bound() {
    let w = long_world(Some(3_000)).await;
    let r = state(&w, BEA, "big", row(&w, WS, "big", 5_001)).await;
    assert_eq!(r.0, 200, "{}", r.1);
    assert_eq!(r.1["nodes"][0]["x"], 5_000.0);
    assert_eq!(w.past.afters.lock().unwrap().first(), Some(&3_000));
    // Below the snapshot the replay starts at 0 and is as far as the file is long.
    let r = state(&w, BEA, "big", row(&w, WS, "big", 2_999)).await;
    assert_eq!(r.0, 200, "{}", r.1);
    assert_eq!(r.1["nodes"][0]["x"], 2_998.0);
}

fn tight(burst: u32) -> Spec {
    Spec {
        per_principal: Some(Rate {
            burst,
            per_second: 1,
        }),
        ..Spec::default()
    }
}

#[tokio::test]
async fn a_replay_costs_eight_calls_and_a_cache_hit_one() {
    // The person's budget is 12: a miss takes 8 (the call's own one included), each hit 1.
    let w = with(tight(12)).await;
    assert_eq!(rest(&w, AGENT, "f", vec![add("a1", "fa")]).await.0, 200);
    let (one, two) = (row(&w, WS, "f", 1), row(&w, WS, "f", 0));
    assert_eq!(state(&w, BEA, "f", one).await.0, 200, "a miss: 8 of 12");
    assert_eq!((w.past.pages(), snapshots(&w)), (1, 1));
    for i in 0..3 {
        assert_eq!(state(&w, BEA, "f", one).await.0, 200, "a hit {i}: one call");
    }
    assert_eq!(w.past.pages(), 1, "the hits read nothing");
    // One call is left: a miss cannot be covered, and nothing is read for it. Then a hit is limited too.
    for n in [two, one] {
        let r = state(&w, BEA, "f", n).await;
        assert_eq!((r.0, code(&r)), (429, "rate_limited"), "{}", r.1);
    }
    assert_eq!((opened(&w), snapshots(&w)), (1, 1));
}

#[tokio::test]
async fn the_byte_bound_evicts_the_oldest_answer() {
    // The size of three answers, measured on a world with room to spare.
    let probe = start().await;
    script(&probe).await;
    let mut sizes = Vec::new();
    for seq in [2, 5, 8] {
        let (_, body) = state(&probe, BEA, "f", row(&probe, WS, "f", seq)).await;
        sizes.push(body.to_string().len());
    }
    // Room for the newest two and not for the three.
    let mut spec = Spec::default();
    spec.limits.bytes = sizes[1] + sizes[2];
    let w = with(spec).await;
    script(&w).await;
    let (a, b, c) = (
        row(&w, WS, "f", 2),
        row(&w, WS, "f", 5),
        row(&w, WS, "f", 8),
    );
    let (_, first) = state(&w, BEA, "f", a).await;
    for n in [b, c] {
        assert_eq!(state(&w, BEA, "f", n).await.0, 200);
    }
    w.past.reset();
    // The newest two are held; the oldest was evicted and is read again.
    assert_eq!(state(&w, BEA, "f", c).await.0, 200);
    assert_eq!(state(&w, BEA, "f", b).await.0, 200);
    assert_eq!(opened(&w), 0);
    assert_eq!(state(&w, BEA, "f", a).await.1, first);
    assert_eq!(opened(&w), 1);
}

#[tokio::test]
async fn an_answer_that_cannot_fit_alone_is_not_kept_and_the_entry_bound_holds() {
    let mut spec = Spec::default();
    spec.limits.bytes = 10;
    let w = with(spec).await;
    script(&w).await;
    let n = row(&w, WS, "f", 8);
    for _ in 0..2 {
        assert_eq!(state(&w, BEA, "f", n).await.0, 200);
    }
    assert_eq!(opened(&w), 2);
    let mut spec = Spec::default();
    spec.limits.entries = 2;
    let w = with(spec).await;
    script(&w).await;
    let rows: Vec<u32> = (1..=3).map(|s| row(&w, WS, "f", s)).collect();
    for n in &rows {
        assert_eq!(state(&w, BEA, "f", n).await.0, 200);
    }
    w.past.reset();
    assert_eq!(state(&w, BEA, "f", rows[2]).await.0, 200);
    assert_eq!(opened(&w), 0);
    assert_eq!(state(&w, BEA, "f", rows[0]).await.0, 200);
    assert_eq!(opened(&w), 1);
}

#[tokio::test]
async fn only_a_listed_person_under_an_auto_policy_may_read_a_state() {
    let w = start().await;
    script(&w).await;
    let n = row(&w, WS, "f", 3);
    let r = state(&w, AGENT, "f", n).await;
    assert_eq!((r.0, code(&r)), (403, "forbidden"), "{}", r.1);
    let (status, _) = get(&w, None, &format!("/v1/files/f/checkpoints/{n}/state")).await;
    assert_eq!(status, 401);
    // Listed, and the policy denied: forbidden before the row is looked at.
    let denied = Spec {
        state_policy: Some(Policy::Deny),
        ..Spec::default()
    };
    let w = with(denied).await;
    script(&w).await;
    let r = state(&w, BEA, "f", row(&w, WS, "f", 3)).await;
    assert_eq!((r.0, code(&r)), (403, "forbidden"));
    let r = state(&w, BEA, "f", 99).await;
    assert_eq!(
        (r.0, code(&r)),
        (403, "forbidden"),
        "it tells nothing of the rows"
    );
    assert_eq!(opened(&w), 0);
    // No entry at all (the module not registered): the same.
    let none = Spec {
        state_policy: None,
        ..Spec::default()
    };
    let w = with(none).await;
    script(&w).await;
    let r = state(&w, BEA, "f", row(&w, WS, "f", 3)).await;
    assert_eq!((r.0, code(&r)), (403, "forbidden"));
}

#[tokio::test]
async fn a_missing_number_and_another_workspaces_file_are_not_found_and_a_bad_number_is_invalid() {
    let spec = Spec {
        may: vec!["user:bea".into(), "agent:gamma".into()],
        ..Spec::default()
    };
    let w = with(spec).await;
    script(&w).await;
    let n = row(&w, WS, "f", 3);
    let missing = state(&w, BEA, "f", n + 1).await;
    assert_eq!((missing.0, code(&missing)), (404, "not_found"));
    // Another workspace asks for the same file and the same number: it has no row, so it is told no more than that.
    let other = state(&w, OTHER, "f", n).await;
    assert_eq!(other, missing);
    assert_eq!(state(&w, BEA, "nothing", 1).await, missing);
    for bad in ["0", "01", "+1", "1.0", "-1", "x", "99999999999999999999"] {
        let r = state(&w, BEA, "f", bad).await;
        assert_eq!((r.0, code(&r)), (400, "invalid"), "{bad}: {}", r.1);
    }
    assert_eq!(opened(&w), 0);
}

#[tokio::test]
async fn the_call_log_holds_the_tool_the_file_and_the_outcome_and_no_number() {
    let w = start().await;
    script(&w).await;
    let n = row(&w, WS, "f", 3);
    assert_eq!(state(&w, BEA, "f", n).await.0, 200);
    assert_eq!(state(&w, BEA, "f", n + 5).await.0, 404);
    assert_eq!(state(&w, AGENT, "f", n).await.0, 403);
    let read = Some(meldframe_module::RiskTier::Read);
    let calls = w.server.calls(100);
    let mine = calls.iter().filter(|e| e.tool == "checkpoint_state");
    let seen: Vec<_> = mine
        .map(|e| (&*e.actor, &*e.outcome, &*e.file, e.risk))
        .collect();
    let expect = [
        ("user:bea", "ok", "f"),
        ("user:bea", "not_found", "f"),
        ("agent:alpha", "forbidden", "f"),
    ];
    let expect: Vec<_> = expect.iter().map(|(a, o, f)| (*a, *o, *f, read)).collect();
    assert_eq!(seen, expect);
}

#[tokio::test]
async fn a_log_with_variable_commands_replays_and_leaves_the_variables_out() {
    let w = start().await;
    let collection = json!({"type": "addCollection", "collection": {"id": "t", "name": "appearance",
        "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}});
    let variable = json!({"type": "addVariable", "variable": {"id": "bg", "collection": "t",
        "name": "color/bg", "type": "color", "values": {"light": "#ffffff", "dark": "#000000"}}});
    let steps = [
        add("a1", "fa"),
        op("v1", collection),
        op("v2", variable),
        patch("a2", "fa", 5.0),
    ];
    for step in steps {
        assert_eq!(rest(&w, AGENT, "f", vec![step]).await.0, 200);
    }
    let log = log_of(&w, "f");
    for seq in 0..=4 {
        let r = state(&w, BEA, "f", row(&w, WS, "f", seq)).await;
        assert_eq!(r.0, 200, "at seq {seq}: {}", r.1);
        assert_eq!(r.1, expected(&log, seq));
    }
}
