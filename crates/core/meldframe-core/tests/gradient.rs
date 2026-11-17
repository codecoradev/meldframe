//! E3.2: a gradient painted over the fill. A node holds none by default; a patch sets, replaces or removes it
//! (`null`); it is validated (2 to 8 stops, positions from 0 to 1 that do not go back, hex colours) and refused on text.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_core::{Limits, apply_command};
use meldframe_protocol::{Command, Node};
use serde_json::{Value, json};

fn apply(nodes: &mut Vec<Node>, v: Value) -> Result<(), meldframe_core::ApplyError> {
    apply_command(
        nodes,
        &serde_json::from_value::<Command>(v).unwrap(),
        &Limits::default(),
    )
}

fn wire(nodes: &[Node], id: &str) -> Value {
    serde_json::to_value(nodes.iter().find(|n| n.id == id).unwrap()).unwrap()
}

fn two_stops() -> Value {
    json!({"kind": "linear", "angle": 90, "stops": [{"pos": 0, "color": "#ff0000"}, {"pos": 1, "color": "#0000ff80"}]})
}

fn doc() -> Vec<Node> {
    let mut ns = vec![];
    apply(
        &mut ns,
        json!({"type": "addNode", "node": {"id": "a", "type": "rect", "fill": "#112233"}}),
    )
    .unwrap();
    ns
}

#[test]
fn a_node_has_no_gradient_until_one_is_set_and_it_is_not_written_when_absent() {
    let ns = doc();
    assert!(ns[0].gradient.is_none());
    assert!(wire(&ns, "a").get("gradient").is_none());
}

#[test]
fn a_patch_sets_replaces_and_removes_a_gradient_without_touching_the_fill() {
    let mut ns = doc();
    apply(
        &mut ns,
        json!({"type": "updateNode", "id": "a", "patch": {"gradient": two_stops()}}),
    )
    .unwrap();
    let w = wire(&ns, "a");
    assert_eq!(w["gradient"]["stops"].as_array().unwrap().len(), 2);
    assert_eq!(
        (w["gradient"]["angle"].clone(), w["fill"].clone()),
        (json!(90.0), json!("#112233"))
    );
    let radial = json!({"kind": "radial", "stops": [{"pos": 0, "color": "#fff"}, {"pos": 0.5, "color": "#888"}, {"pos": 1, "color": "#000"}]});
    apply(
        &mut ns,
        json!({"type": "updateNode", "id": "a", "patch": {"gradient": radial}}),
    )
    .unwrap();
    assert_eq!(wire(&ns, "a")["gradient"]["kind"], json!("radial"));
    apply(
        &mut ns,
        json!({"type": "updateNode", "id": "a", "patch": {"gradient": null}}),
    )
    .unwrap();
    assert!(wire(&ns, "a").get("gradient").is_none());
    assert_eq!(wire(&ns, "a")["fill"], json!("#112233"));
}

#[test]
fn a_patch_that_leaves_the_gradient_out_keeps_it() {
    let mut ns = doc();
    apply(
        &mut ns,
        json!({"type": "updateNode", "id": "a", "patch": {"gradient": two_stops()}}),
    )
    .unwrap();
    apply(
        &mut ns,
        json!({"type": "updateNode", "id": "a", "patch": {"x": 5}}),
    )
    .unwrap();
    assert!(ns[0].gradient.is_some());
}

#[test]
fn a_gradient_that_is_not_valid_is_refused_naming_what_is_wrong_and_changes_nothing() {
    let mut ns = doc();
    let before = serde_json::to_string(&ns).unwrap();
    let one = json!({"stops": [{"pos": 0, "color": "#fff"}]});
    let nine: Vec<Value> = (0..9)
        .map(|i| json!({"pos": i as f64 / 8.0, "color": "#fff"}))
        .collect();
    let cases = [
        (one, "stops"),
        (json!({"stops": nine}), "stops"),
        (
            json!({"stops": [{"pos": -0.1, "color": "#fff"}, {"pos": 1, "color": "#000"}]}),
            "pos",
        ),
        (
            json!({"stops": [{"pos": 0, "color": "#fff"}, {"pos": 1.5, "color": "#000"}]}),
            "pos",
        ),
        (
            json!({"stops": [{"pos": 0.8, "color": "#fff"}, {"pos": 0.2, "color": "#000"}]}),
            "order",
        ),
        (
            json!({"stops": [{"pos": 0, "color": "red"}, {"pos": 1, "color": "#000"}]}),
            "colour",
        ),
        (
            json!({"angle": 1.0e9, "stops": [{"pos": 0, "color": "#fff"}, {"pos": 1, "color": "#000"}]}),
            "angle",
        ),
    ];
    for (g, word) in cases {
        let e = apply(
            &mut ns,
            json!({"type": "updateNode", "id": "a", "patch": {"gradient": g.clone()}}),
        )
        .unwrap_err();
        assert!(e.to_string().contains(word), "{g}: {e}");
        let e = apply(
            &mut ns,
            json!({"type": "addNode", "node": {"id": "b", "type": "rect", "gradient": g}}),
        )
        .unwrap_err();
        assert!(e.to_string().contains(word), "{e}");
    }
    assert_eq!(serde_json::to_string(&ns).unwrap(), before);
}

#[test]
fn equal_positions_are_allowed_for_a_hard_edge() {
    let mut ns = doc();
    let hard = json!({"stops": [{"pos": 0, "color": "#fff"}, {"pos": 0.5, "color": "#fff"}, {"pos": 0.5, "color": "#000"}, {"pos": 1, "color": "#000"}]});
    apply(
        &mut ns,
        json!({"type": "updateNode", "id": "a", "patch": {"gradient": hard}}),
    )
    .unwrap();
}

#[test]
fn text_takes_no_gradient() {
    let mut ns = vec![];
    let e = apply(
        &mut ns,
        json!({"type": "addNode", "node": {"id": "t", "type": "text", "gradient": two_stops()}}),
    )
    .unwrap_err();
    assert_eq!(e.code(), "E_GRADIENT_ON_TEXT");
    apply(
        &mut ns,
        json!({"type": "addNode", "node": {"id": "t", "type": "text"}}),
    )
    .unwrap();
    let e = apply(
        &mut ns,
        json!({"type": "updateNode", "id": "t", "patch": {"gradient": two_stops()}}),
    )
    .unwrap_err();
    assert_eq!(e.code(), "E_GRADIENT_ON_TEXT");
}

#[test]
fn a_node_saved_before_gradients_existed_has_none() {
    let n: Node = serde_json::from_str(r##"{"id":"old","type":"frame"}"##).unwrap();
    assert!(n.gradient.is_none());
}
