//! E3.2: opacity and no fill through the core's submit-a-command interface. Expected values are literals.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_core::{Limits, apply_command};
use meldframe_protocol::{Command, Node};
use serde_json::{Value, json};

fn cmd(v: Value) -> Command {
    serde_json::from_value(v).unwrap()
}

fn apply(nodes: &mut Vec<Node>, v: Value) -> Result<(), meldframe_core::ApplyError> {
    apply_command(nodes, &cmd(v), &Limits::default())
}

fn wire(nodes: &[Node], id: &str) -> Value {
    serde_json::to_value(nodes.iter().find(|n| n.id == id).unwrap()).unwrap()
}

#[test]
fn opacity_defaults_to_one_and_a_patch_sets_it_without_touching_other_fields() {
    let mut ns = vec![];
    apply(&mut ns, json!({"type": "addNode", "node": {"id": "a", "type": "rect", "fill": "#112233", "radius": 4}})).unwrap();
    assert_eq!(wire(&ns, "a")["opacity"], json!(1.0));
    apply(
        &mut ns,
        json!({"type": "updateNode", "id": "a", "patch": {"opacity": 0.5, "fill": "none"}}),
    )
    .unwrap();
    let w = wire(&ns, "a");
    assert_eq!(
        (w["opacity"].clone(), w["fill"].clone(), w["radius"].clone()),
        (json!(0.5), json!("none"), json!(4.0))
    );
}

#[test]
fn an_opacity_out_of_range_is_refused_and_leaves_the_nodes_alone() {
    let mut ns = vec![];
    apply(
        &mut ns,
        json!({"type": "addNode", "node": {"id": "a", "type": "rect"}}),
    )
    .unwrap();
    let before = serde_json::to_string(&ns).unwrap();
    for bad in [-0.5, 1.5] {
        let e = apply(
            &mut ns,
            json!({"type": "updateNode", "id": "a", "patch": {"opacity": bad}}),
        )
        .unwrap_err();
        assert!(e.to_string().contains("opacity"), "{e}");
    }
    assert_eq!(serde_json::to_string(&ns).unwrap(), before);
}

#[test]
fn a_node_saved_before_opacity_existed_loads_as_opaque() {
    let n: Node =
        serde_json::from_str(r##"{"id":"old","type":"frame","fill":"#000000"}"##).unwrap();
    assert_eq!(n.opacity, 1.0);
}
