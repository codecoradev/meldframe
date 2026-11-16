//! Review fixes of the MCP surface: what an atomic batch must not write, `ifSeq` on every node a command
//! touches, bounded scans, conflict details, the `Host` allow-list and one room open per read.

mod common;
use common::*;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;

fn rect(id: &str) -> Value {
    json!({"type": "addNode", "node": {"id": id, "type": "rect", "name": id, "w": 10, "h": 10}})
}

fn child(id: &str, parent: &str) -> Value {
    text(id, parent, "t")
}

fn remove(id: &str) -> Value {
    json!({"type": "removeNode", "id": id})
}

fn reparent(id: &str, parent: &str) -> Value {
    json!({"type": "reparent", "id": id, "parentId": parent})
}

async fn apply(c: &Client, file: &str, key: &str, commands: Vec<Value>, extra: Value) -> Called {
    let mut args = json!({"file": file, "idempotencyKey": key, "commands": commands});
    for (k, v) in extra.as_object().into_iter().flatten() {
        args[k] = v.clone();
    }
    call(c, "apply_commands", args).await
}

async fn seq(c: &Client, file: &str) -> u64 {
    call(c, "get_tree", json!({"file": file})).await.body["seq"]
        .as_u64()
        .unwrap()
}

/// Why a remove cannot write half a batch: only a frame can hold children, a frame delete is never small,
/// and a batch that adds or reparents gets no small-delete relief, so the gate refuses the whole batch
/// before command 0. These are the three batches that were suspected of a partial write.
#[tokio::test]
async fn a_remove_in_a_batch_that_grows_the_tree_writes_nothing() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    apply(&c, "r3", "s", vec![frame("l", "L"), rect("x")], json!({})).await;
    let cases: [Vec<Value>; 3] = [
        vec![child("c", "l"), remove("l")],
        vec![frame("f", "F"), remove("f")],
        vec![reparent("x", "l"), remove("l")],
    ];
    for (i, cmds) in cases.into_iter().enumerate() {
        let r = apply(&c, "r3", &format!("b{i}"), cmds, json!({})).await;
        assert!(r.is_error, "case {i}: {}", r.body);
        assert_eq!(r.body["error"]["code"], "forbidden", "case {i}: {}", r.body);
        assert_eq!(seq(&c, "r3").await, 2, "case {i} wrote a command");
    }
}

#[tokio::test]
async fn a_small_leaf_delete_still_goes_through_in_a_batch_without_growth() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    apply(&c, "r4", "s", vec![rect("a"), rect("b")], json!({})).await;
    let r = apply(
        &c,
        "r4",
        "d",
        vec![mv("a", 1.0, 1.0), remove("b")],
        json!({}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(seq(&c, "r4").await, 4);
}

#[tokio::test]
async fn if_seq_covers_a_node_a_mixed_command_touches_for_the_first_time() {
    let h = start().await;
    let agent = connect(&h, AGENT).await;
    let person = connect(&h, PERSON).await;
    apply(
        &agent,
        "i1",
        "s",
        vec![frame("p", "P"), frame("q", "Q"), child("x", "q")],
        json!({}),
    )
    .await;
    let seen = seq(&agent, "i1").await;
    let moved = apply(&person, "i1", "m", vec![mv("x", 5.0, 5.0)], json!({})).await;
    assert!(!moved.is_error, "{}", moved.body);
    let rename = json!({"type": "updateNode", "id": "p", "patch": {"name": "P2"}});
    let r = apply(
        &agent,
        "i1",
        "a",
        vec![rename, reparent("x", "p")],
        json!({"ifSeq": seen}),
    )
    .await;
    assert!(r.is_error, "the stale move of x went through: {}", r.body);
    assert_eq!(r.body["error"]["code"], "conflict");
    assert_eq!(r.body["error"]["nodeId"], "x");
    assert_eq!(
        seq(&agent, "i1").await,
        4,
        "nothing of the call was written"
    );
}

#[tokio::test]
async fn an_if_seq_far_behind_the_file_is_refused_not_scanned() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    for n in 0..21 {
        let cmds: Vec<Value> = (0..100).map(|i| frame(&format!("n{n}_{i}"), "n")).collect();
        let r = apply(&c, "s1", &format!("k{n}"), cmds, json!({})).await;
        assert!(!r.is_error, "{}", r.body);
    }
    let r = apply(
        &c,
        "s1",
        "late",
        vec![mv("n0_0", 1.0, 1.0)],
        json!({"ifSeq": 0}),
    )
    .await;
    assert!(r.is_error, "{}", r.body);
    assert_eq!(r.body["error"]["code"], "invalid", "{}", r.body);
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("ifSeq")
    );
    assert_eq!(seq(&c, "s1").await, 2100);
    let now = seq(&c, "s1").await;
    let ok = apply(
        &c,
        "s1",
        "fresh",
        vec![mv("n0_0", 1.0, 1.0)],
        json!({"ifSeq": now}),
    )
    .await;
    assert!(!ok.is_error, "{}", ok.body);
}

#[tokio::test]
async fn a_conflict_names_the_change_even_when_it_is_far_after_if_seq() {
    let h = start().await;
    let agent = connect(&h, AGENT).await;
    let person = connect(&h, PERSON).await;
    apply(
        &agent,
        "c1",
        "s",
        vec![frame("h", "H"), frame("o", "O")],
        json!({}),
    )
    .await;
    let seen = seq(&agent, "c1").await;
    let noise: Vec<Value> = (0..100).map(|i| mv("o", i as f64, 0.0)).collect();
    apply(&person, "c1", "noise", noise, json!({})).await;
    apply(&person, "c1", "real", vec![mv("h", 7.0, 7.0)], json!({})).await;
    let r = apply(
        &agent,
        "c1",
        "a",
        vec![mv("h", 1.0, 1.0)],
        json!({"ifSeq": seen}),
    )
    .await;
    assert_eq!(r.body["error"]["code"], "conflict", "{}", r.body);
    let changes = r.body["error"]["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1, "{}", r.body);
    assert_eq!(changes[0]["actor"], "user:bea");
    assert_eq!(r.body["error"]["currentSeq"], seen + 101);
}

#[tokio::test]
async fn a_read_and_a_conflict_each_open_the_room_once() {
    let h = start().await;
    let agent = connect(&h, AGENT).await;
    apply(&agent, "o1", "s", vec![frame("h", "H")], json!({})).await;
    let before = h.spy.reads.load(Ordering::SeqCst);
    call(&agent, "get_tree", json!({"file": "o1"})).await;
    assert_eq!(h.spy.reads.load(Ordering::SeqCst) - before, 1);
    let before = h.spy.reads.load(Ordering::SeqCst);
    let third = apply(
        &agent,
        "o1",
        "c",
        vec![mv("h", 3.0, 3.0)],
        json!({"ifSeq": 0}),
    )
    .await;
    assert_eq!(third.body["error"]["code"], "conflict");
    assert_eq!(h.spy.reads.load(Ordering::SeqCst) - before, 1);
}

async fn list_status(h: &Harness, host: &str) -> u16 {
    reqwest::Client::new()
        .post(format!("http://{}/mcp", h.addr))
        .header("host", host)
        .header("accept", "application/json, text/event-stream")
        .bearer_auth(AGENT)
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

#[tokio::test]
async fn configured_hosts_extend_the_loopback_names_and_others_are_refused() {
    let plain = start().await;
    assert_eq!(list_status(&plain, "localhost").await, 200);
    assert_eq!(list_status(&plain, "mcp.example.test").await, 403);
    let h = start_hosts(config(), vec!["mcp.example.test".into()]).await;
    assert_eq!(list_status(&h, "mcp.example.test").await, 200);
    for loopback in ["localhost", "127.0.0.1"] {
        assert_eq!(list_status(&h, loopback).await, 200, "{loopback}");
    }
    assert_eq!(list_status(&h, "evil.example.test").await, 403);
}

#[tokio::test]
async fn a_mixed_command_cannot_be_guarded_when_the_call_is_not_atomic() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    apply(&c, "n1", "s", vec![frame("p", "P"), rect("x")], json!({})).await;
    let rename = json!({"type": "updateNode", "id": "p", "patch": {"name": "P2"}});
    let r = apply(
        &c,
        "n1",
        "a",
        vec![rename, reparent("x", "p")],
        json!({"ifSeq": 2, "atomic": false}),
    )
    .await;
    assert_eq!(r.body["error"]["code"], "invalid", "{}", r.body);
    assert_eq!(r.body["error"]["index"], 1);
    assert_eq!(seq(&c, "n1").await, 2, "nothing was written");
}
