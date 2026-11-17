//! E3.2: margin per side through the core: validated, applied by a patch, defaults to 0, and a file from before it
//! existed loads with no margin.

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

#[test]
fn margins_default_to_zero_and_a_patch_sets_one_side_only() {
    let mut ns = vec![];
    apply(
        &mut ns,
        json!({"type": "addNode", "node": {"id": "a", "type": "rect", "radius": 4}}),
    )
    .unwrap();
    let w = wire(&ns, "a");
    assert_eq!(
        (w["marT"].clone(), w["marL"].clone()),
        (json!(0.0), json!(0.0))
    );
    apply(
        &mut ns,
        json!({"type": "updateNode", "id": "a", "patch": {"marL": 6}}),
    )
    .unwrap();
    let w = wire(&ns, "a");
    assert_eq!(
        (w["marL"].clone(), w["marR"].clone(), w["radius"].clone()),
        (json!(6.0), json!(0.0), json!(4.0))
    );
}

#[test]
fn a_negative_or_huge_margin_is_refused_naming_the_side_and_leaves_the_nodes_alone() {
    let mut ns = vec![];
    apply(
        &mut ns,
        json!({"type": "addNode", "node": {"id": "a", "type": "rect"}}),
    )
    .unwrap();
    let before = serde_json::to_string(&ns).unwrap();
    for (side, bad) in [
        ("marT", json!(-1)),
        ("marR", json!(1.0e12)),
        ("marB", json!(-0.5)),
        ("marL", json!(-9)),
    ] {
        let e = apply(
            &mut ns,
            json!({"type": "updateNode", "id": "a", "patch": {side: bad}}),
        )
        .unwrap_err();
        assert!(e.to_string().contains(side), "{side}: {e}");
        let e = apply(
            &mut ns,
            json!({"type": "addNode", "node": {"id": "b", "type": "rect", side: -1}}),
        )
        .unwrap_err();
        assert!(e.to_string().contains(side), "{side}: {e}");
    }
    assert_eq!(serde_json::to_string(&ns).unwrap(), before);
}

#[test]
fn a_node_saved_before_margins_existed_loads_with_none() {
    let n: Node = serde_json::from_str(r##"{"id":"old","type":"frame"}"##).unwrap();
    let m = n.margin();
    assert_eq!((m.top, m.right, m.bottom, m.left), (0.0, 0.0, 0.0, 0.0));
}
