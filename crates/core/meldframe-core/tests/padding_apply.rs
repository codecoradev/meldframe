//! E3.2: padding per side through the core's submit-a-command interface; expected values are literals.

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
fn a_patch_sets_one_side_and_leaves_the_others_and_the_rest_alone() {
    let mut ns = vec![];
    apply(&mut ns, json!({"type": "addNode", "node": {"id": "a", "type": "frame", "padding": 16, "radius": 4}})).unwrap();
    assert!(
        wire(&ns, "a").get("padT").is_none(),
        "an unset side is not written"
    );
    apply(
        &mut ns,
        json!({"type": "updateNode", "id": "a", "patch": {"padL": 3, "padR": 9}}),
    )
    .unwrap();
    let w = wire(&ns, "a");
    assert_eq!(
        (
            w["padL"].clone(),
            w["padR"].clone(),
            w["padding"].clone(),
            w["radius"].clone()
        ),
        (json!(3.0), json!(9.0), json!(16.0), json!(4.0))
    );
    assert!(w.get("padT").is_none() && w.get("padB").is_none());
}

#[test]
fn a_bad_side_is_refused_and_leaves_the_nodes_alone() {
    let mut ns = vec![];
    apply(
        &mut ns,
        json!({"type": "addNode", "node": {"id": "a", "type": "frame"}}),
    )
    .unwrap();
    let before = serde_json::to_string(&ns).unwrap();
    let e = apply(
        &mut ns,
        json!({"type": "updateNode", "id": "a", "patch": {"padT": -4}}),
    )
    .unwrap_err();
    assert!(e.to_string().contains("padT"), "{e}");
    assert_eq!(serde_json::to_string(&ns).unwrap(), before);
}

#[test]
fn null_unsets_a_side_so_it_takes_the_single_padding_again() {
    let mut ns = vec![];
    apply(
        &mut ns,
        json!({"type": "addNode", "node": {"id": "a", "type": "frame", "padding": 16, "padL": 3}}),
    )
    .unwrap();
    apply(
        &mut ns,
        json!({"type": "updateNode", "id": "a", "patch": {"padL": null}}),
    )
    .unwrap();
    let w = wire(&ns, "a");
    assert!(w.get("padL").is_none(), "{w}");
    assert_eq!(ns[0].pad().left, 16.0);
}
