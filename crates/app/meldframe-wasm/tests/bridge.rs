//! The JSON boundary: what the browser sends and reads, exercised natively on the plain-Rust `Bridge`.

use meldframe_wasm::Bridge;
use serde_json::{Value, json};

fn bridge() -> Bridge {
    Bridge::new("usr_me", "web", 7, "[]", 0).expect("empty file loads")
}

fn run(b: &mut Bridge, action: Value) -> Value {
    serde_json::from_str(&b.dispatch(&action.to_string()).expect("dispatch")).expect("json")
}

#[test]
fn start_asks_for_a_subscription_from_the_last_confirmed_number() {
    let mut b = bridge();
    let fx = run(&mut b, json!({"type": "start", "nowMs": 1}));
    assert_eq!(fx, json!([{"type": "subscribe", "afterSeq": 0}]));
}

#[test]
fn create_node_changes_nodes_state_and_events() {
    let mut b = bridge();
    run(&mut b, json!({"type": "start", "nowMs": 1}));
    run(&mut b, json!({"type": "connected", "nowMs": 2}));
    let fx = run(
        &mut b,
        json!({"type": "createNode", "kind": "rectangle", "x": 10.0, "y": 20.0, "parent": null}),
    );
    assert!(
        fx.as_array()
            .unwrap()
            .iter()
            .any(|e| e["type"] == "sendOps")
    );
    let nodes: Value = serde_json::from_str(&b.nodes_json()).unwrap();
    assert_eq!(nodes.as_array().unwrap().len(), 1);
    let state: Value = serde_json::from_str(&b.state_json()).unwrap();
    assert_eq!(state["selection"].as_array().unwrap().len(), 1);
    assert_eq!(state["history"]["canUndo"], true);
    let events: Value = serde_json::from_str(&b.take_events_json()).unwrap();
    assert!(
        events
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["type"] == "documentChanged")
    );
    // Taking drains.
    assert_eq!(b.take_events_json(), "[]");
}

#[test]
fn bad_json_and_unknown_actions_are_errors_not_panics() {
    let mut b = bridge();
    assert!(b.dispatch("not json").is_err());
    assert!(b.dispatch(r#"{"type":"nope"}"#).is_err());
}

#[test]
fn a_file_that_does_not_load_is_an_error() {
    assert!(Bridge::new("usr_me", "web", 1, "{", 0).is_err());
}

#[test]
fn a_file_that_does_not_load_names_its_stable_code() {
    // Take a valid node list, then repeat the node so that the ids clash.
    let mut b = bridge();
    run(&mut b, json!({"type": "start", "nowMs": 1}));
    run(&mut b, json!({"type": "connected", "nowMs": 2}));
    run(
        &mut b,
        json!({"type": "createNode", "kind": "frame", "x": 0.0, "y": 0.0, "parent": null}),
    );
    let nodes: Vec<Value> = serde_json::from_str(&b.nodes_json()).unwrap();
    let doubled = json!([nodes[0], nodes[0]]).to_string();
    let err = Bridge::new("usr_me", "web", 1, &doubled, 0).unwrap_err();
    assert_eq!(err, "load: E_DUPLICATE_ID");
}

#[test]
fn whole_numbers_are_checked_not_coerced() {
    use meldframe_wasm::whole_number;
    assert_eq!(whole_number("seq", 0.0, 10.0), Ok(0));
    assert_eq!(whole_number("seq", 10.0, 10.0), Ok(10));
    for bad in [-1.0, 0.5, 11.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            whole_number("seq", bad, 10.0).is_err(),
            "{bad} must be refused"
        );
    }
    assert_eq!(whole_number("seq", -0.0, 10.0), Ok(0));
}

#[test]
fn the_scene_is_the_painted_boxes_of_the_document_as_json() {
    let mut b = bridge();
    assert_eq!(b.scene_json(), "[]");
    run(&mut b, json!({"type": "start", "nowMs": 1}));
    run(&mut b, json!({"type": "connected", "nowMs": 2}));
    run(
        &mut b,
        json!({"type": "createNode", "kind": "frame", "x": 10.0, "y": 20.0, "parent": null}),
    );
    // The new node is still pending (not confirmed by the server), and the scene already shows it.
    let nodes: Vec<Value> = serde_json::from_str(&b.nodes_json()).unwrap();
    let scene: Vec<Value> = serde_json::from_str(&b.scene_json()).unwrap();
    assert_eq!(scene.len(), 1);
    assert_eq!(scene[0]["id"], nodes[0]["id"]);
    assert_eq!(scene[0]["paint"], "frame");
    assert_eq!(
        (scene[0]["x"].as_f64(), scene[0]["y"].as_f64()),
        (Some(10.0), Some(20.0))
    );
    assert_eq!(scene[0]["parent"], Value::Null);
}
