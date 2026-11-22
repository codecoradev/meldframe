//! The checkpoints across the JSON boundary (E5.3, ADR-0023, slice 8): the actions, frames, effects and state the web
//! client sends and reads, through the plain-Rust `Bridge`. The rules are tested at the app seam (`meldframe-app-review-
//! tests`); this file checks that the boundary carries them as the generated TypeScript types say.

use meldframe_wasm::Bridge;
use serde_json::{Value, json};

const T: u64 = 1_700_000_000_000;

fn file() -> String {
    json!([{"id": "r", "type": "rect", "pos": "a0", "x": 0, "y": 0, "w": 10, "h": 10}]).to_string()
}

fn bridge() -> Bridge {
    let mut b = Bridge::new("user:me", "web", 7, &file(), 0, None).expect("the file loads");
    run(&mut b, json!({"type": "start", "nowMs": T}));
    run(&mut b, json!({"type": "connected", "nowMs": T + 1}));
    b
}

fn run(b: &mut Bridge, action: Value) -> Value {
    serde_json::from_str(&b.dispatch(&action.to_string()).expect("dispatch")).expect("json")
}

fn state(b: &Bridge) -> Value {
    serde_json::from_str(&b.state_json()).expect("json")
}

fn row(n: u32, seq: u64, kind: &str) -> Value {
    let mut r = json!({"n": n, "seq": seq, "kind": kind, "atMs": T});
    if kind == "runStart" {
        r["about"] = json!("agent:x");
    } else {
        r["by"] = json!("user:me");
    }
    r
}

fn loaded(b: &mut Bridge, rows: Vec<Value>, can_create: bool) {
    run(
        b,
        json!({"type": "checkpointsLoaded", "checkpoints": rows, "canCreate": can_create}),
    );
}

/// The one effect of this type in `fx`.
fn effect(fx: &Value, kind: &str) -> Value {
    let found: Vec<&Value> = fx
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == kind)
        .collect();
    assert_eq!(found.len(), 1, "{kind} in {fx}");
    found[0].clone()
}

#[test]
fn a_subscribe_reads_the_list_and_the_state_starts_with_none() {
    let mut b = Bridge::new("user:me", "web", 7, &file(), 0, None).unwrap();
    run(&mut b, json!({"type": "start", "nowMs": T}));
    let fx = run(&mut b, json!({"type": "connected", "nowMs": T + 1}));
    assert_eq!(
        effect(&fx, "fetchCheckpoints"),
        json!({"type": "fetchCheckpoints"})
    );
    let s = state(&b);
    assert_eq!(
        (s["checkpoints"].clone(), s["canCreateCheckpoint"].clone()),
        (json!([]), json!(false))
    );
    assert_eq!(s["checkpointsStatus"], "unread");
    loaded(&mut b, vec![], true);
    assert_eq!(state(&b)["checkpointsStatus"], "loaded");
    let gone = json!({"type": "checkpointRequestFailed", "request": {"type": "list"}, "code": "not_found"});
    run(&mut b, gone);
    let s = state(&b);
    assert_eq!(
        (
            s["checkpointsStatus"].clone(),
            s["canCreateCheckpoint"].clone()
        ),
        (json!("absent"), json!(false))
    );
}

#[test]
fn a_loaded_list_and_a_frame_reach_the_state_newest_first_with_the_text_of_n_as_the_id() {
    let mut b = bridge();
    loaded(
        &mut b,
        vec![row(1, 0, "runStart"), row(2, 3, "manual")],
        true,
    );
    let frame = json!({"type": "frame", "nowMs": T, "frame": {"type": "checkpoint", "checkpoint": row(3, 4, "beforeRestore")}});
    let fx = run(&mut b, frame);
    assert_eq!(fx, json!([]), "n 3 follows 2: nothing to read");
    let s = state(&b);
    assert_eq!(s["canCreateCheckpoint"], true);
    let ids: Vec<&str> = s["checkpoints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["3", "2", "1"]);
    assert_eq!(
        s["checkpoints"][2],
        json!({"id": "1", "n": 1, "seq": 0, "kind": "runStart", "about": "agent:x", "atMs": T})
    );
    let skip = json!({"type": "frame", "nowMs": T, "frame": {"type": "checkpoint", "checkpoint": row(9, 4, "manual")}});
    assert_eq!(
        effect(&run(&mut b, skip), "fetchCheckpoints"),
        json!({"type": "fetchCheckpoints"})
    );
}

#[test]
fn a_manual_checkpoint_goes_out_as_a_post_and_joins_when_the_row_comes_back() {
    let mut b = bridge();
    loaded(&mut b, vec![], true);
    let fx = run(
        &mut b,
        json!({"type": "createCheckpoint", "label": " Before the pitch "}),
    );
    let post = effect(&fx, "postCheckpoint");
    assert_eq!(
        (
            post["kind"].clone(),
            post["label"].clone(),
            post["about"].clone(),
            post["seq"].clone()
        ),
        (
            json!("manual"),
            json!("Before the pitch"),
            Value::Null,
            json!(0)
        )
    );
    assert_eq!(
        state(&b)["checkpoints"],
        json!([]),
        "not before the server says"
    );
    let mut made = row(1, 0, "manual");
    made["label"] = json!("Before the pitch");
    run(
        &mut b,
        json!({"type": "checkpointPosted", "key": post["key"], "checkpoint": made}),
    );
    assert_eq!(state(&b)["checkpoints"][0]["label"], "Before the pitch");
    let bad = run_err(
        &mut b,
        json!({"type": "createCheckpoint", "label": "pay\u{202e}now"}),
    );
    assert_eq!(
        bad, None,
        "a label that is not fit is a refusal, not an error"
    );
    let events: Value = serde_json::from_str(&b.take_events_json()).unwrap();
    assert!(
        events
            .as_array()
            .unwrap()
            .contains(&json!({"type": "refused", "code": "invalid"}))
    );
}

fn run_err(b: &mut Bridge, action: Value) -> Option<String> {
    b.dispatch(&action.to_string()).err()
}

fn agent_moves_r(b: &mut Bridge) {
    let command = json!({"type": "updateNode", "id": "r", "patch": {"x": 50}});
    let event =
        json!({"type": "event", "seq": 1, "actor": "agent:x", "key": "b1:0", "command": command});
    run(b, json!({"type": "frame", "nowMs": T, "frame": event}));
}

#[test]
fn a_revert_waits_for_its_row_across_the_boundary() {
    let mut b = bridge();
    loaded(&mut b, vec![], true);
    agent_moves_r(&mut b);
    let fx = run(
        &mut b,
        json!({"type": "revertBatch", "actor": "agent:x", "keyPrefix": "b1"}),
    );
    let post = effect(&fx, "postCheckpoint");
    assert_eq!(
        (
            post["kind"].clone(),
            post["about"].clone(),
            post["seq"].clone()
        ),
        (json!("beforeRevert"), json!("agent:x"), json!(1))
    );
    assert!(
        fx.as_array()
            .unwrap()
            .iter()
            .all(|e| e["type"] != "sendOps"),
        "nothing is sent before the row"
    );
    let fx = run(
        &mut b,
        json!({"type": "checkpointPosted", "key": post["key"], "checkpoint": row(1, 1, "beforeRevert")}),
    );
    let sent = effect(&fx, "sendOps");
    assert_eq!(sent["ops"][0]["command"]["patch"]["x"], 0.0);
}

#[test]
fn a_restore_reads_the_file_at_the_checkpoint_posts_a_row_and_sends_the_change() {
    let mut b = bridge();
    loaded(&mut b, vec![row(1, 0, "manual")], true);
    agent_moves_r(&mut b);
    let fx = run(&mut b, json!({"type": "restoreCheckpoint", "id": "1"}));
    assert_eq!(
        effect(&fx, "fetchCheckpointState"),
        json!({"type": "fetchCheckpointState", "n": 1})
    );
    let before: Value = serde_json::from_str(&b.nodes_json()).unwrap();
    let mut at = before.clone();
    at[0]["x"] = json!(0);
    let loaded_state = json!({"type": "checkpointStateLoaded", "n": 1, "seq": 0, "nodes": at});
    let fx = run(&mut b, loaded_state);
    let post = effect(&fx, "postCheckpoint");
    assert_eq!(
        (post["kind"].clone(), post["seq"].clone()),
        (json!("beforeRestore"), json!(1))
    );
    let fx = run(
        &mut b,
        json!({"type": "checkpointPosted", "key": post["key"], "checkpoint": row(2, 1, "beforeRestore")}),
    );
    assert_eq!(
        effect(&fx, "sendOps")["ops"][0]["command"]["patch"]["x"],
        0.0
    );
}

#[test]
fn a_failed_request_refuses_what_waited_and_bad_payloads_are_errors_not_panics() {
    let mut b = bridge();
    loaded(&mut b, vec![], true);
    agent_moves_r(&mut b);
    let fx = run(
        &mut b,
        json!({"type": "revertBatch", "actor": "agent:x", "keyPrefix": "b1"}),
    );
    let key = effect(&fx, "postCheckpoint")["key"].clone();
    b.take_events_json();
    run(
        &mut b,
        json!({"type": "checkpointRequestFailed", "request": {"type": "post", "key": key}, "code": "timeout"}),
    );
    let events: Value = serde_json::from_str(&b.take_events_json()).unwrap();
    assert_eq!(
        events,
        json!([{"type": "refused", "code": "checkpoint_failed"}])
    );
    for action in [
        json!({"type": "checkpointsLoaded", "checkpoints": [{"n": 1, "seq": 0, "kind": "snapshot", "atMs": 1}], "canCreate": true}),
        json!({"type": "checkpointsLoaded", "checkpoints": [{"n": 1, "seq": 0, "kind": "manual", "atMs": 1, "x": 1}], "canCreate": true}),
        json!({"type": "checkpointsLoaded", "checkpoints": []}),
        json!({"type": "checkpointRequestFailed", "request": {"type": "state"}, "code": "x"}),
        json!({"type": "checkpointRequestFailed", "request": {"type": "elsewhere"}, "code": "x"}),
        json!({"type": "checkpointStateLoaded", "n": -1, "seq": 0, "nodes": []}),
        json!({"type": "createCheckpoint", "label": "v1", "x": 1}),
    ] {
        let e = run_err(&mut b, action).expect("refused");
        assert!(e.starts_with("action:"), "{e}");
    }
    let bad_frame = json!({"type": "frame", "nowMs": T, "frame": {"type": "checkpoint", "checkpoint": {"n": 1}}});
    assert!(run_err(&mut b, bad_frame).unwrap().starts_with("action:"));
}
