//! E3.2: `absolute` through the core: false by default, set by a patch, and a file from before it existed has it false.

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

#[test]
fn absolute_defaults_to_false_and_a_patch_sets_it() {
    let mut ns = vec![];
    apply(
        &mut ns,
        json!({"type": "addNode", "node": {"id": "a", "type": "rect", "radius": 4}}),
    )
    .unwrap();
    assert!(!ns[0].absolute);
    apply(
        &mut ns,
        json!({"type": "updateNode", "id": "a", "patch": {"absolute": true}}),
    )
    .unwrap();
    assert!(ns[0].absolute);
    assert_eq!(ns[0].radius, 4.0);
}

#[test]
fn a_node_saved_before_absolute_existed_is_in_the_flow() {
    let n: Node = serde_json::from_str(r##"{"id":"old","type":"rect"}"##).unwrap();
    assert!(!n.absolute);
}
