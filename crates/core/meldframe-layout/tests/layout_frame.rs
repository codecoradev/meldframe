//! The one rule of which node lays out its children: a frame with a row or column layout. The app layer, the export
//! and the panels read this predicate and keep no copy of it.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::node;
use meldframe_layout::is_layout_frame;
use serde_json::json;

#[test]
fn a_frame_with_a_row_or_column_layout_lays_out_its_children() {
    for mode in ["row", "column"] {
        let n = node("f", "frame", None, json!({"layout": mode}));
        assert!(is_layout_frame(&n), "{mode}");
    }
}

#[test]
fn a_frame_with_no_layout_leaves_its_children_where_they_are() {
    assert!(!is_layout_frame(&node("f", "frame", None, json!({}))));
    assert!(!is_layout_frame(&node(
        "f",
        "frame",
        None,
        json!({"layout": "none"})
    )));
}

#[test]
fn only_a_frame_lays_out_whatever_its_layout_field_says() {
    for kind in ["rect", "ellipse", "text", "shader"] {
        let n = node("n", kind, None, json!({"layout": "row"}));
        assert!(!is_layout_frame(&n), "{kind}");
    }
}
