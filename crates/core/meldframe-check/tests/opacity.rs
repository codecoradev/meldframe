//! E3.2: opacity (0 to 1) and no fill (`fill: "none"`). Every refusal names the field.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_check::{ApplyError, Limits, check_node, check_patch};
use meldframe_protocol::{Node, NodePatch};
use serde_json::{Value, json};

fn node(extra: Value) -> Node {
    let mut v = json!({"id": "n", "type": "rect"});
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

fn patch(v: Value) -> NodePatch {
    serde_json::from_value(v).unwrap()
}

#[test]
fn opacity_is_from_zero_to_one() {
    let l = Limits::default();
    for ok in [0.0, 0.25, 1.0] {
        assert_eq!(
            check_node(&node(json!({"opacity": ok})), &l),
            Ok(()),
            "{ok}"
        );
        assert_eq!(
            check_patch(&patch(json!({"opacity": ok})), &l),
            Ok(()),
            "{ok}"
        );
    }
    for bad in [-0.1, 1.01, 2.0, 100.0] {
        let e = check_node(&node(json!({"opacity": bad})), &l);
        assert!(
            matches!(
                e,
                Err(ApplyError::OutOfRange {
                    field: "opacity",
                    ..
                })
            ),
            "{bad}: {e:?}"
        );
        let e = check_patch(&patch(json!({"opacity": bad})), &l);
        assert!(
            matches!(
                e,
                Err(ApplyError::OutOfRange {
                    field: "opacity",
                    ..
                })
            ),
            "{bad}: {e:?}"
        );
    }
}

#[test]
fn a_node_without_opacity_is_opaque() {
    assert_eq!(node(json!({})).opacity, 1.0);
}

#[test]
fn fill_none_means_no_fill_and_is_accepted() {
    let l = Limits::default();
    assert_eq!(check_node(&node(json!({"fill": "none"})), &l), Ok(()));
    assert_eq!(check_patch(&patch(json!({"fill": "none"})), &l), Ok(()));
}

#[test]
fn other_words_are_still_not_a_colour() {
    let l = Limits::default();
    for bad in ["None", "NONE", "transparent", "no", ""] {
        assert_eq!(
            check_patch(&patch(json!({"fill": bad})), &l),
            Err(ApplyError::InvalidColor { field: "fill" }),
            "{bad:?}"
        );
    }
    // The second colour and the border colour stay hex only: `none` is for the fill.
    assert_eq!(
        check_patch(&patch(json!({"fill2": "none"})), &l),
        Err(ApplyError::InvalidColor { field: "fill2" })
    );
}
