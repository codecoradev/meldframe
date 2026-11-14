//! Justify (main axis) and align (cross axis) of children inside a layout frame.
//! Every expected number is worked out by hand in the comments.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{node, r, rect_of};
use meldframe_protocol::Node;
use serde_json::json;

/// A row 200 x 50 with padding 10 and gap 10 holding two children 30 wide.
fn row_with(justify: &str) -> [Node; 3] {
    [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "row", "padding": 10, "gap": 10, "w": 200, "h": 50, "justify": justify }),
        ),
        node("a", "rect", Some("f"), json!({ "w": 30, "h": 10 })),
        node("b", "rect", Some("f"), json!({ "w": 30, "h": 10 })),
    ]
}

#[test]
fn justify_start_packs_children_at_the_start() {
    let nodes = row_with("start");
    assert_eq!(rect_of(&nodes, "a").x, 10.0);
    assert_eq!(rect_of(&nodes, "b").x, 50.0);
}

#[test]
fn justify_moves_the_group_by_the_space_left_over() {
    // Inner width 180; the children (60) and one gap (10) leave 110.
    let center = row_with("center");
    // Offset 55: a at 10 + 55, b after a and the gap.
    assert_eq!(rect_of(&center, "a").x, 65.0);
    assert_eq!(rect_of(&center, "b").x, 105.0);
    let end = row_with("end");
    // Offset 110.
    assert_eq!(rect_of(&end, "a").x, 120.0);
    assert_eq!(rect_of(&end, "b").x, 160.0);
}

#[test]
fn justify_between_spreads_the_left_over_space_into_the_gaps() {
    let nodes = row_with("between");
    // Extra gap 110 with two children: b at 10 + 30 + 10 + 110 = 160.
    assert_eq!(rect_of(&nodes, "a").x, 10.0);
    assert_eq!(rect_of(&nodes, "b").x, 160.0);
}

#[test]
fn justify_between_with_one_child_leaves_it_at_the_start() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "row", "padding": 10, "w": 200, "h": 50, "justify": "between" }),
        ),
        node("a", "rect", Some("f"), json!({ "w": 30, "h": 10 })),
    ];
    assert_eq!(rect_of(&nodes, "a").x, 10.0);
}

#[test]
fn justify_has_no_effect_when_a_child_fills_the_main_axis() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "row", "padding": 10, "gap": 10, "w": 200, "h": 50, "justify": "center" }),
        ),
        node("a", "rect", Some("f"), json!({ "w": 30, "h": 10 })),
        node("b", "rect", Some("f"), json!({ "sw": "fill", "h": 10 })),
    ];
    // b takes 180 - 30 - 10 = 140 and starts after a: 10 + 30 + 10 = 50.
    assert_eq!(rect_of(&nodes, "a").x, 10.0);
    assert_eq!(rect_of(&nodes, "b"), r(50.0, 10.0, 140.0, 10.0));
}

#[test]
fn justify_never_pushes_children_before_the_padding_when_they_overflow() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "row", "padding": 10, "w": 50, "h": 50, "justify": "center" }),
        ),
        node("a", "rect", Some("f"), json!({ "w": 100, "h": 10 })),
    ];
    assert_eq!(rect_of(&nodes, "a").x, 10.0);
}

#[test]
fn align_places_children_across_a_row() {
    let at = |align: &str| {
        [
            node(
                "f",
                "frame",
                None,
                json!({ "layout": "row", "padding": 10, "w": 200, "h": 100, "align": align }),
            ),
            node("a", "rect", Some("f"), json!({ "w": 30, "h": 20 })),
        ]
    };
    // Inner height 80, child 20: free 60.
    assert_eq!(rect_of(&at("start"), "a").y, 10.0);
    assert_eq!(rect_of(&at("center"), "a").y, 40.0);
    assert_eq!(rect_of(&at("end"), "a").y, 70.0);
}

#[test]
fn align_places_children_across_a_column() {
    let at = |align: &str| {
        [
            node(
                "f",
                "frame",
                None,
                json!({ "layout": "column", "padding": 0, "w": 100, "h": 100, "align": align }),
            ),
            node("a", "rect", Some("f"), json!({ "w": 40, "h": 20 })),
        ]
    };
    assert_eq!(rect_of(&at("start"), "a").x, 0.0);
    assert_eq!(rect_of(&at("center"), "a").x, 30.0);
    assert_eq!(rect_of(&at("end"), "a").x, 60.0);
}
