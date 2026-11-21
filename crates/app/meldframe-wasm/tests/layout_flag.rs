//! The host flag of the canvas crosses the boundary: `set_layout_on_canvas` turns the laid-out scene on and off, answers
//! the effects to carry out, and changes nothing else (no state, no events, no nodes).

use meldframe_wasm::Bridge;
use serde_json::{Value, json};

fn bridge() -> Bridge {
    let nodes = json!([
        {"id": "f", "type": "frame", "w": 200, "h": 300, "layout": "column", "padding": 10, "gap": 5},
        {"id": "a", "type": "rect", "parentId": "f", "pos": "a", "w": 50, "h": 20},
        {"id": "b", "type": "rect", "parentId": "f", "pos": "b", "w": 1, "h": 30, "sw": "fill"},
    ]);
    Bridge::new("usr_me", "web", 7, &nodes.to_string(), 0, None).expect("the file loads")
}

fn scene(b: &Bridge) -> Vec<Value> {
    serde_json::from_str(&b.scene_json()).unwrap()
}

fn box_of(scene: &[Value], id: &str) -> [f64; 4] {
    let b = scene.iter().find(|b| b["id"] == id).expect("a box");
    ["x", "y", "w", "h"].map(|k| b[k].as_f64().unwrap())
}

#[test]
fn the_scene_is_the_stored_boxes_until_the_host_turns_the_layout_on() {
    let b = bridge();
    assert_eq!(box_of(&scene(&b), "b"), [0.0, 0.0, 1.0, 30.0]);
}

#[test]
fn the_flag_gives_the_laid_out_boxes_and_asks_for_a_redraw() {
    let mut b = bridge();
    let off = b.scene_json();
    let fx: Value = serde_json::from_str(&b.set_layout_on_canvas(true)).unwrap();
    assert_eq!(fx, json!([{"type": "render", "nodes": []}]));
    let on = scene(&b);
    assert_eq!(box_of(&on, "a"), [10.0, 10.0, 50.0, 20.0]);
    assert_eq!(box_of(&on, "b"), [10.0, 35.0, 180.0, 30.0]);
    // Asking for what is already so costs nothing.
    assert_eq!(b.set_layout_on_canvas(true), "[]");
    assert_eq!(
        b.set_layout_on_canvas(false),
        r#"[{"type":"render","nodes":[]}]"#
    );
    assert_eq!(
        b.scene_json(),
        off,
        "off again: the stored boxes, byte for byte"
    );
}

#[test]
fn the_flag_is_not_state_not_an_event_and_not_the_file() {
    let mut b = bridge();
    let (state, nodes) = (b.state_json(), b.nodes_json());
    b.set_layout_on_canvas(true);
    assert_eq!(b.state_json(), state);
    assert_eq!(b.nodes_json(), nodes);
    assert_eq!(b.take_events_json(), "[]");
}

#[test]
fn the_drop_preview_is_null_until_the_layout_is_on_and_then_a_slot_and_the_nodes_that_follow_as_json()
 {
    let mut b = bridge();
    // `b` is drawn from y 35 to 65 once laid out; `a` is dragged to a pointer past its middle.
    assert_eq!(
        b.drop_preview_json(r#"["a"]"#, (30.0, 60.0), (0.0, 0.0)),
        "null"
    );
    b.set_layout_on_canvas(true);
    let slot: Value =
        serde_json::from_str(&b.drop_preview_json(r#"["a"]"#, (30.0, 60.0), (0.0, 0.0))).unwrap();
    assert_eq!(
        slot,
        json!({"slot": {"frame": "f", "index": 1, "line": {"x": 10.0, "y": 65.0, "w": 180.0, "h": 0.0}}, "follow": []})
    );
    let out: Value =
        serde_json::from_str(&b.drop_preview_json(r#"["a"]"#, (1500.0, 900.0), (0.0, 0.0)))
            .unwrap();
    assert_eq!(
        out,
        json!({"slot": null, "follow": ["a"]}),
        "out of the flow: no slot, the node follows"
    );
    assert_eq!(
        b.drop_preview_json("not json", (0.0, 0.0), (0.0, 0.0)),
        "null"
    );
    assert_eq!(b.take_events_json(), "[]", "a preview raises no event");
}
