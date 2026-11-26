//! The raw answers and frames across the JSON boundary (audit finding T6, issue 323): the `wire` action the web client
//! sends with the status and the text it received, through the plain-Rust `Bridge`. The rules of each answer are tested at
//! the app seam (`meldframe-app-unit-tests`, `answers_*.rs`); this file checks that the boundary carries the shapes of the
//! generated `Wire` type (camelCase `kind`, `nowMs`, the key of a post) and reaches the state and the events.

use meldframe_wasm::Bridge;
use serde_json::{Value, json};

const T: u64 = 1_700_000_000_000;

fn bridge() -> Bridge {
    let file = json!([{"id": "r", "type": "rect", "pos": "a0", "x": 0, "y": 0, "w": 10, "h": 10}]);
    let mut b =
        Bridge::new("user:me", "web", 7, &file.to_string(), 0, None).expect("the file loads");
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

fn events(b: &mut Bridge) -> Vec<Value> {
    serde_json::from_str(&b.take_events_json()).expect("json")
}

fn wire(b: &mut Bridge, w: Value) {
    run(b, json!({"type": "wire", "wire": w}));
}

fn header(n: u32) -> Value {
    json!({"n": n, "rev": 1, "actor": "agent:x", "keyPrefix": "k", "tool": "apply_commands", "commands": 1,
        "written": 1, "atomic": false, "firstSeq": 1, "lastSeq": 1, "atMs": T, "review": "unreviewed"})
}

#[test]
fn a_batch_list_as_raw_text_is_a_page_and_a_bad_one_is_refused_as_unavailable() {
    let mut b = bridge();
    let body = json!({"batches": [header(1)], "hasMore": false, "unreviewedCount": 1, "canReview": true, "added": 0});
    wire(
        &mut b,
        json!({"kind": "batches", "status": 200, "body": body.to_string()}),
    );
    let s = state(&b);
    assert_eq!(s["activity"]["unreviewedCount"], 1);
    assert_eq!(s["activity"]["canReview"], true);
    events(&mut b);
    wire(
        &mut b,
        json!({"kind": "batches", "status": 200, "body": "<html>"}),
    );
    let e = events(&mut b);
    assert!(
        e.iter()
            .any(|e| e["type"] == "refused" && e["code"] == "unavailable"),
        "{e:?}"
    );
    assert_eq!(state(&b)["activity"]["unreviewedCount"], 1);
}

#[test]
fn a_hint_frame_as_raw_text_with_the_time_it_came_is_taken_and_a_bad_one_is_lost() {
    let mut b = bridge();
    let mut f = header(2);
    f["type"] = json!("batch");
    f["room"] = json!("file_dev");
    f["unreviewedCount"] = json!(4);
    wire(
        &mut b,
        json!({"kind": "hint", "nowMs": T + 5, "text": f.to_string()}),
    );
    assert_eq!(state(&b)["activity"]["unreviewedCount"], 4);
    events(&mut b);
    f["n"] = json!(0);
    f["unreviewedCount"] = json!(9);
    wire(
        &mut b,
        json!({"kind": "hint", "nowMs": T + 6, "text": f.to_string()}),
    );
    assert!(
        events(&mut b).is_empty(),
        "a hint that does not pass is ignored"
    );
    assert_eq!(state(&b)["activity"]["unreviewedCount"], 4);
}

#[test]
fn a_post_answer_and_a_list_as_raw_text_are_read_into_the_state() {
    let mut b = bridge();
    let row = json!({"n": 1, "seq": 0, "kind": "manual", "by": "user:me", "atMs": T});
    // The row of a post is in the list at once, read from the raw text of the answer.
    let body = json!({"checkpoint": row, "created": true}).to_string();
    wire(
        &mut b,
        json!({"kind": "checkpointPosted", "key": "k1", "status": 200, "body": body}),
    );
    assert_eq!(state(&b)["checkpoints"].as_array().unwrap().len(), 1);
    // A list as raw text is read the same way, with a field it does not know ignored.
    let second = json!({"n": 2, "seq": 0, "kind": "manual", "by": "user:me", "atMs": T});
    let list = json!({"checkpoints": [second, row], "canCreate": true, "more": 1}).to_string();
    wire(
        &mut b,
        json!({"kind": "checkpoints", "status": 200, "body": list}),
    );
    assert_eq!(state(&b)["checkpoints"].as_array().unwrap().len(), 2);
    assert_eq!(state(&b)["canCreateCheckpoint"], true);
}

#[test]
fn the_shape_of_wire_is_closed_so_a_field_it_does_not_have_or_a_kind_it_does_not_know_is_refused() {
    let mut b = bridge();
    for bad in [
        json!({"type": "wire", "wire": {"kind": "batches", "status": 200, "body": "", "x": 1}}),
        json!({"type": "wire", "wire": {"kind": "nope", "status": 200, "body": ""}}),
        json!({"type": "wire", "wire": {"kind": "batches", "status": 200}}),
    ] {
        assert!(b.dispatch(&bad.to_string()).is_err(), "{bad}");
    }
}
