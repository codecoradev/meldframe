//! The Activity panel across the JSON boundary (E5.1, ADR-0021, slice 10): the actions, frames, effects and state the
//! web client sends and reads, through the plain-Rust `Bridge`. The rules are tested in `meldframe-activity-tests` and
//! at the app seam; this file checks that the boundary carries them as the generated TypeScript types say.

use meldframe_wasm::Bridge;
use serde_json::{Value, json};

const T: u64 = 1_700_000_000_000;

fn bridge() -> Bridge {
    let mut b = Bridge::new("user:me", "web", 7, "[]", 0, None).expect("empty file loads");
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

fn events(b: &mut Bridge) -> Vec<String> {
    let v: Value = serde_json::from_str(&b.take_events_json()).expect("json");
    let list = v.as_array().expect("array").iter();
    list.map(|e| e["type"].as_str().unwrap().to_owned())
        .collect()
}

fn header(n: u32, rev: u64, prefix: &str, note: Value) -> Value {
    let mut h = json!({"n": n, "rev": rev, "actor": "agent:x", "keyPrefix": prefix, "tool": "apply_commands",
        "commands": 1, "written": 1, "atomic": false, "firstSeq": 1, "lastSeq": 1, "atMs": T,
        "review": "unreviewed"});
    if !note.is_null() {
        h["note"] = note;
    }
    h
}

fn page(batches: Vec<Value>, count: u32, can_review: bool) -> Value {
    json!({"batches": batches, "hasMore": false, "unreviewedCount": count, "canReview": can_review})
}

fn frame(h: Value, count: u32) -> Value {
    json!({"type": "frame", "nowMs": T + 5, "frame": {"type": "batch", "header": h, "unreviewedCount": count}})
}

#[test]
fn a_fresh_file_has_an_empty_panel_in_the_state() {
    let b = bridge();
    let a = &state(&b)["activity"];
    for key in ["lanes", "rows", "touches"] {
        assert_eq!(a[key], json!([]), "{key}");
    }
    // Nothing was read yet: the count is unknown, not zero (rule U9).
    assert_eq!(a["unreviewedCount"], Value::Null);
    assert_eq!(a["canReview"], false);
    assert_eq!(a["hasOlder"], false);
    assert_eq!(a["selected"], Value::Null);
}

#[test]
fn opening_the_tab_asks_for_the_newest_page_as_an_effect() {
    let mut b = bridge();
    let fx = run(&mut b, json!({"type": "openActivity"}));
    assert_eq!(
        fx,
        json!([{"type": "fetchBatches", "beforeN": null, "afterN": null, "limit": 100}])
    );
}

#[test]
fn a_batch_frame_makes_a_row_with_its_note_and_only_the_panel_changes() {
    let mut b = bridge();
    events(&mut b);
    run(&mut b, frame(header(1, 1, "b1", json!("Hero")), 1));
    let a = state(&b)["activity"].clone();
    assert_eq!(a["unreviewedCount"], 1);
    assert_eq!(a["rows"][0]["n"], 1);
    assert_eq!(a["rows"][0]["note"], "Hero");
    assert_eq!(a["rows"][0]["status"], "applied");
    assert_eq!(a["lanes"][0]["actor"], "agent:x");
    assert_eq!(a["lanes"][0]["view"]["kind"], "agent");
    assert_eq!(a["lanes"][0]["view"]["name"], "x");
    assert_eq!(events(&mut b), ["activityChanged"]);
    assert_eq!(b.take_events_json(), "[]");
}

#[test]
fn a_note_reaches_the_state_without_its_hidden_text() {
    let mut b = bridge();
    run(
        &mut b,
        frame(header(1, 1, "b1", json!("safe\u{202e}evil")), 1),
    );
    let row = &state(&b)["activity"]["rows"][0];
    assert_eq!(row["n"], 1, "the row is kept");
    assert_eq!(
        row["note"], "safeevil",
        "the bidi override is stripped (the second check)"
    );
}

#[test]
fn a_touch_frame_draws_a_ring_for_another_actor() {
    let mut b = bridge();
    let touch = json!({"type": "frame", "nowMs": T, "frame":
        {"type": "touch", "actor": "agent:x", "ids": ["a", "b"], "seq": 3}});
    run(&mut b, touch);
    let rings = state(&b)["activity"]["touches"].clone();
    assert_eq!(rings[0]["actor"], "agent:x");
    assert_eq!(rings[0]["ids"], json!(["a", "b"]));
    assert_eq!(rings[0]["persistent"], false);
}

#[test]
fn a_loaded_page_sets_the_count_and_whether_the_person_may_review() {
    let mut b = bridge();
    let rows = vec![
        header(2, 1, "b2", Value::Null),
        header(1, 1, "b1", Value::Null),
    ];
    let fx = run(
        &mut b,
        json!({"type": "batchesLoaded", "page": page(rows, 2, true)}),
    );
    assert_eq!(fx, json!([]));
    let a = &state(&b)["activity"];
    assert_eq!(a["canReview"], true);
    assert_eq!(a["unreviewedCount"], 2);
    let order: Vec<&Value> = a["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| &r["n"])
        .collect();
    assert_eq!(order, [&json!(2), &json!(1)], "newest first");
}

#[test]
fn a_review_goes_out_as_an_effect_and_the_row_changes_only_when_the_answer_comes_back() {
    let mut b = bridge();
    run(
        &mut b,
        json!({"type": "batchesLoaded", "page": page(vec![header(1, 1, "b1", Value::Null)], 1, true)}),
    );
    let batch = json!({"actor": "agent:x", "keyPrefix": "b1"});
    let fx = run(
        &mut b,
        json!({"type": "setBatchReview", "batch": batch, "state": "needs_review"}),
    );
    assert_eq!(
        fx,
        json!([{"type": "putBatchReview", "n": 1, "state": "needs_review"}])
    );
    assert_eq!(state(&b)["activity"]["rows"][0]["review"], "unreviewed");
    let mut answer = header(1, 2, "b1", Value::Null);
    answer["review"] = json!("needs_review");
    answer["reviewedBy"] = json!("user:me");
    answer["reviewedAtMs"] = json!(T + 9);
    run(&mut b, json!({"type": "batchReviewed", "batch": answer}));
    let row = &state(&b)["activity"]["rows"][0];
    assert_eq!(row["review"], "needs_review");
    assert_eq!(row["reviewedBy"], "user:me");
}

#[test]
fn selecting_a_batch_and_a_failed_request_cross_the_boundary() {
    let mut b = bridge();
    run(
        &mut b,
        json!({"type": "batchesLoaded", "page": page(vec![header(1, 1, "b1", Value::Null)], 1, false)}),
    );
    run(
        &mut b,
        json!({"type": "selectBatch", "batch": {"actor": "agent:x", "keyPrefix": "b1"}}),
    );
    assert_eq!(
        state(&b)["activity"]["selected"],
        json!({"actor": "agent:x", "keyPrefix": "b1"})
    );
    events(&mut b);
    run(
        &mut b,
        json!({"type": "batchRequestFailed", "code": "forbidden"}),
    );
    let drained: Value = serde_json::from_str(&b.take_events_json()).expect("json");
    assert_eq!(
        drained,
        json!([{"type": "refused", "code": "forbidden"}]),
        "a failed request is told to the front end as a refusal with the server's code"
    );
}

#[test]
fn unknown_fields_and_bad_values_are_errors_not_panics() {
    let mut b = bridge();
    for action in [
        json!({"type": "openActivity", "x": 1}),
        json!({"type": "setBatchReview", "batch": {"actor": "a", "keyPrefix": "k"}, "state": "approved"}),
        json!({"type": "batchesLoaded", "page": {"batches": [], "hasMore": false, "unreviewedCount": 0}}),
        json!({"type": "batchReviewed", "batch": {"n": -1}}),
    ] {
        let e = b.dispatch(&action.to_string()).expect_err("refused");
        assert!(e.starts_with("action:"), "{e}");
    }
    // A header is read as the server sends it: a field the app does not know is ignored, not an error (audit finding T6).
    let mut h = header(1, 1, "b1", Value::Null);
    h["extra"] = json!(true);
    b.dispatch(&frame(h, 1).to_string())
        .expect("a field more is ignored");
}
