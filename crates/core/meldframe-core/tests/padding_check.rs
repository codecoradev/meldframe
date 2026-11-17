//! E3.2: padding per side is a number from 0 up to the size limit, on a node and in a patch; every refusal names
//! the side.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_check::{ApplyError, Limits, check_node, check_patch};
use meldframe_protocol::{Node, NodePatch};
use serde_json::{Value, json};

fn node(extra: Value) -> Node {
    let mut v = json!({"id": "n", "type": "frame"});
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

fn patch(v: Value) -> NodePatch {
    serde_json::from_value(v).unwrap()
}

#[test]
fn each_side_accepts_zero_and_a_normal_value() {
    let l = Limits::default();
    for side in ["padT", "padR", "padB", "padL"] {
        assert_eq!(check_node(&node(json!({side: 0})), &l), Ok(()), "{side}");
        assert_eq!(
            check_patch(&patch(json!({side: 24.5})), &l),
            Ok(()),
            "{side}"
        );
    }
}

#[test]
fn a_negative_or_huge_side_is_refused_and_names_the_side() {
    let l = Limits::default();
    for side in ["padT", "padR", "padB", "padL"] {
        for bad in [json!(-1), json!(1.0e12)] {
            let e = check_patch(&patch(json!({side: bad.clone()})), &l);
            assert!(
                matches!(&e, Err(ApplyError::OutOfRange { field, .. }) if *field == side),
                "{side} {bad}: {e:?}"
            );
            let e = check_node(&node(json!({side: bad.clone()})), &l);
            assert!(
                matches!(&e, Err(ApplyError::OutOfRange { field, .. }) if *field == side),
                "{side} {bad}: {e:?}"
            );
        }
    }
}

#[test]
fn a_node_without_sides_reads_the_single_padding_on_every_side() {
    let p = node(json!({"padding": 7})).pad();
    assert_eq!((p.top, p.right, p.bottom, p.left), (7.0, 7.0, 7.0, 7.0));
    let p = node(json!({"padding": 7, "padR": 1, "padB": 2})).pad();
    assert_eq!((p.top, p.right, p.bottom, p.left), (7.0, 1.0, 2.0, 7.0));
}
