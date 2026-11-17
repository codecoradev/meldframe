//! Fill sizes: children that share the free space on the main axis and stretch across.
//! Every expected number is worked out by hand in the comments.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{node, r, rect_of};
use serde_json::json;

#[test]
fn a_fill_child_takes_the_space_the_fixed_children_leave() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "row", "padding": 10, "gap": 5, "w": 300, "h": 100 }),
        ),
        node("a", "rect", Some("f"), json!({ "w": 50, "h": 20 })),
        node("b", "rect", Some("f"), json!({ "sw": "fill", "h": 30 })),
    ];
    // Inner width 280; minus a (50) and one gap (5) leaves 225.
    assert_eq!(rect_of(&nodes, "a"), r(10.0, 10.0, 50.0, 20.0));
    assert_eq!(rect_of(&nodes, "b"), r(65.0, 10.0, 225.0, 30.0));
}

#[test]
fn fill_children_share_the_free_space_equally() {
    let fill = |id: &str| {
        node(
            id,
            "rect",
            Some("f"),
            json!({ "sw": "fill", "w": 50, "h": 10 }),
        )
    };
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "row", "padding": 0, "gap": 5, "w": 205, "h": 50 }),
        ),
        fill("a"),
        fill("b"),
    ];
    // (205 - 5) / 2 = 100 each; b starts at 100 + 5.
    assert_eq!(rect_of(&nodes, "a"), r(0.0, 0.0, 100.0, 10.0));
    assert_eq!(rect_of(&nodes, "b"), r(105.0, 0.0, 100.0, 10.0));
}

#[test]
fn a_fill_child_in_a_column_takes_the_remaining_height() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "column", "padding": 10, "gap": 0, "w": 100, "h": 200 }),
        ),
        node("a", "rect", Some("f"), json!({ "w": 10, "h": 40 })),
        node("b", "rect", Some("f"), json!({ "w": 10, "sh": "fill" })),
    ];
    // Inner height 180 minus a (40) leaves 140; b starts at 10 + 40.
    assert_eq!(rect_of(&nodes, "b"), r(10.0, 50.0, 10.0, 140.0));
}

#[test]
fn fill_stretches_across_the_axis() {
    let row = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "row", "padding": 10, "w": 300, "h": 100 }),
        ),
        node("a", "rect", Some("f"), json!({ "w": 20, "sh": "fill" })),
    ];
    // Inner height 100 - 20 = 80.
    assert_eq!(rect_of(&row, "a"), r(10.0, 10.0, 20.0, 80.0));
    let column = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "column", "padding": 10, "w": 200, "h": 100 }),
        ),
        node("a", "rect", Some("f"), json!({ "sw": "fill", "h": 20 })),
    ];
    assert_eq!(rect_of(&column, "a"), r(10.0, 10.0, 180.0, 20.0));
}

#[test]
fn fill_gets_nothing_when_the_fixed_children_use_all_the_space() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "row", "padding": 0, "gap": 0, "w": 100, "h": 50 }),
        ),
        node("a", "rect", Some("f"), json!({ "w": 150, "h": 10 })),
        node("b", "rect", Some("f"), json!({ "sw": "fill", "h": 10 })),
    ];
    assert_eq!(rect_of(&nodes, "b"), r(150.0, 0.0, 0.0, 10.0));
}

#[test]
fn fill_passes_down_through_nested_frames() {
    let nodes = [
        node(
            "outer",
            "frame",
            None,
            json!({ "layout": "column", "padding": 0, "w": 200, "h": 200 }),
        ),
        node(
            "inner",
            "frame",
            Some("outer"),
            json!({ "layout": "row", "padding": 10, "sw": "fill", "sh": "fill" }),
        ),
        node("c", "rect", Some("inner"), json!({ "sw": "fill", "h": 10 })),
    ];
    assert_eq!(rect_of(&nodes, "inner"), r(0.0, 0.0, 200.0, 200.0));
    assert_eq!(rect_of(&nodes, "c"), r(10.0, 10.0, 180.0, 10.0));
}

#[test]
fn a_fill_child_under_a_hug_parent_keeps_its_own_size() {
    // Rule G1: fill where the parent hugs is treated as fixed.
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "row", "sw": "hug", "sh": "hug", "padding": 5 }),
        ),
        node(
            "a",
            "rect",
            Some("f"),
            json!({ "sw": "fill", "w": 40, "h": 10 }),
        ),
    ];
    // Hug width 40 + 10 = 50; the child gets the inner 40, equal to its own width.
    assert_eq!(rect_of(&nodes, "f"), r(0.0, 0.0, 50.0, 20.0));
    assert_eq!(rect_of(&nodes, "a"), r(5.0, 5.0, 40.0, 10.0));
}

#[test]
fn hug_height_follows_the_wrapped_height_of_a_fill_width_text_in_a_column() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "column", "sh": "hug", "padding": 10, "w": 45 }),
        ),
        node(
            "t",
            "text",
            Some("f"),
            json!({ "sw": "fill", "sh": "hug", "text": "aa bb cc", "fontSize": 10 }),
        ),
    ];
    // Inner width 25: "aa bb" fits, "cc" wraps, so 2 lines of 12 = 24; frame height 24 + 20 = 44.
    assert_eq!(rect_of(&nodes, "t"), r(10.0, 10.0, 25.0, 24.0));
    assert_eq!(rect_of(&nodes, "f"), r(0.0, 0.0, 45.0, 44.0));
}

#[test]
fn hug_height_of_a_row_uses_the_final_fill_width_of_a_wrapping_text() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "row", "sh": "hug", "padding": 5, "gap": 0, "w": 65 }),
        ),
        node("a", "rect", Some("f"), json!({ "w": 30, "h": 10 })),
        node(
            "t",
            "text",
            Some("f"),
            json!({ "sw": "fill", "sh": "hug", "text": "aa bb cc", "fontSize": 10 }),
        ),
    ];
    // Inner width 55; minus a (30) leaves 25 for the text, which wraps to 24. Height max(10, 24) + 10 = 34.
    assert_eq!(rect_of(&nodes, "t"), r(35.0, 5.0, 25.0, 24.0));
    assert_eq!(rect_of(&nodes, "f"), r(0.0, 0.0, 65.0, 34.0));
}
