//! Layout on one axis: absolute children, row and column stacking, gap, padding, fixed and hug sizes.
//! Every expected number is worked out by hand in the comments.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{node, r, rect_of};
use meldframe_layout::{StubMeasurer, compute_rects};
use serde_json::json;

#[test]
fn children_of_a_frame_without_layout_sit_at_their_own_offsets() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "x": 10, "y": 20, "w": 100, "h": 50 }),
        ),
        node(
            "c",
            "rect",
            Some("f"),
            json!({ "x": 5, "y": 7, "w": 20, "h": 10 }),
        ),
    ];
    // Child: (10 + 5, 20 + 7), size 20 x 10.
    assert_eq!(rect_of(&nodes, "f"), r(10.0, 20.0, 100.0, 50.0));
    assert_eq!(rect_of(&nodes, "c"), r(15.0, 27.0, 20.0, 10.0));
}

#[test]
fn a_node_with_the_id_root_is_an_ordinary_node() {
    let nodes = [
        node(
            "page",
            "frame",
            None,
            json!({ "x": 100, "y": 100, "w": 300, "h": 300 }),
        ),
        node(
            "root",
            "frame",
            Some("page"),
            json!({ "x": 10, "y": 10, "w": 80, "h": 80 }),
        ),
        node(
            "leaf",
            "rect",
            Some("root"),
            json!({ "x": 1, "y": 2, "w": 3, "h": 4 }),
        ),
    ];
    // 100 + 10 + 1 and 100 + 10 + 2.
    assert_eq!(rect_of(&nodes, "leaf"), r(111.0, 112.0, 3.0, 4.0));
    let top = [node(
        "root",
        "frame",
        None,
        json!({ "x": 10, "y": 20, "w": 100, "h": 50 }),
    )];
    assert_eq!(rect_of(&top, "root"), r(10.0, 20.0, 100.0, 50.0));
}

#[test]
fn a_column_stacks_children_down_from_the_padding_with_gaps() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "column", "padding": 10, "gap": 4, "w": 200, "h": 200 }),
        ),
        node("a", "rect", Some("f"), json!({ "w": 50, "h": 20 })),
        node("b", "rect", Some("f"), json!({ "w": 60, "h": 30 })),
    ];
    // a at (10, 10); b below it: 10 + 20 + 4 = 34.
    assert_eq!(rect_of(&nodes, "a"), r(10.0, 10.0, 50.0, 20.0));
    assert_eq!(rect_of(&nodes, "b"), r(10.0, 34.0, 60.0, 30.0));
}

#[test]
fn a_row_places_children_left_to_right() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "x": 5, "y": 6, "layout": "row", "padding": 10, "gap": 5, "w": 300, "h": 100 }),
        ),
        node("a", "rect", Some("f"), json!({ "w": 30, "h": 10 })),
        node("b", "rect", Some("f"), json!({ "w": 40, "h": 20 })),
    ];
    // a at (5 + 10, 6 + 10); b after a: 15 + 30 + 5 = 50.
    assert_eq!(rect_of(&nodes, "a"), r(15.0, 16.0, 30.0, 10.0));
    assert_eq!(rect_of(&nodes, "b"), r(50.0, 16.0, 40.0, 20.0));
}

#[test]
fn a_hug_row_sums_its_children_on_the_main_axis_and_takes_the_tallest_across() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "row", "sw": "hug", "sh": "hug", "padding": 10, "gap": 5 }),
        ),
        node("a", "rect", Some("f"), json!({ "w": 30, "h": 10 })),
        node("b", "rect", Some("f"), json!({ "w": 40, "h": 20 })),
    ];
    // Width 30 + 5 + 40 + 2 * 10 = 95; height max(10, 20) + 2 * 10 = 40.
    assert_eq!(rect_of(&nodes, "f"), r(0.0, 0.0, 95.0, 40.0));
}

#[test]
fn a_hug_column_takes_the_widest_child_and_sums_heights_with_gaps() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "column", "sw": "hug", "sh": "hug", "padding": 10, "gap": 4 }),
        ),
        node("a", "rect", Some("f"), json!({ "w": 50, "h": 20 })),
        node("b", "rect", Some("f"), json!({ "w": 60, "h": 30 })),
    ];
    // Width max(50, 60) + 20 = 80; height 20 + 4 + 30 + 20 = 74.
    assert_eq!(rect_of(&nodes, "f"), r(0.0, 0.0, 80.0, 74.0));
}

#[test]
fn an_empty_hug_frame_shrinks_to_its_padding() {
    let nodes = [node(
        "f",
        "frame",
        None,
        json!({ "layout": "row", "sw": "hug", "sh": "hug", "padding": 10 }),
    )];
    assert_eq!(rect_of(&nodes, "f"), r(0.0, 0.0, 20.0, 20.0));
}

#[test]
fn hug_nests_inside_hug() {
    let nodes = [
        node(
            "outer",
            "frame",
            None,
            json!({ "layout": "column", "sw": "hug", "sh": "hug", "padding": 8, "gap": 0 }),
        ),
        node(
            "inner",
            "frame",
            Some("outer"),
            json!({ "layout": "row", "sw": "hug", "sh": "hug", "padding": 4, "gap": 2 }),
        ),
        node("a", "rect", Some("inner"), json!({ "w": 10, "h": 10 })),
        node("b", "rect", Some("inner"), json!({ "w": 10, "h": 10 })),
    ];
    // inner: 10 + 2 + 10 + 8 = 30 wide, 10 + 8 = 18 high. outer: 30 + 16 = 46 wide, 18 + 16 = 34 high.
    assert_eq!(rect_of(&nodes, "outer"), r(0.0, 0.0, 46.0, 34.0));
    assert_eq!(rect_of(&nodes, "inner"), r(8.0, 8.0, 30.0, 18.0));
    assert_eq!(rect_of(&nodes, "a"), r(12.0, 12.0, 10.0, 10.0));
    assert_eq!(rect_of(&nodes, "b"), r(24.0, 12.0, 10.0, 10.0));
}

#[test]
fn hug_text_is_sized_by_the_measurer() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({ "layout": "column", "sw": "hug", "sh": "hug", "padding": 0 }),
        ),
        node(
            "t",
            "text",
            Some("f"),
            json!({ "sw": "hug", "sh": "hug", "text": "hello world", "fontSize": 10 }),
        ),
    ];
    // Stub: 11 chars * 5 = 55 wide, one line of 12.
    assert_eq!(rect_of(&nodes, "t"), r(0.0, 0.0, 55.0, 12.0));
    assert_eq!(rect_of(&nodes, "f"), r(0.0, 0.0, 55.0, 12.0));
}

#[test]
fn text_with_a_fixed_width_and_hug_height_wraps_at_that_width() {
    let text = |w: f64| {
        [
            node(
                "f",
                "frame",
                None,
                json!({ "layout": "column", "padding": 0, "w": 200, "h": 200 }),
            ),
            node(
                "t",
                "text",
                Some("f"),
                json!({ "w": w, "sh": "hug", "text": "aa bb cc", "fontSize": 10 }),
            ),
        ]
    };
    // 5 px per char: "aa bb" is 25 px. At width 25 "cc" wraps (2 lines of 12); at 40 all fits (1 line).
    assert_eq!(rect_of(&text(25.0), "t"), r(0.0, 0.0, 25.0, 24.0));
    assert_eq!(rect_of(&text(40.0), "t"), r(0.0, 0.0, 40.0, 12.0));
}

#[test]
fn rects_come_parent_before_child_and_siblings_in_order_of_position_then_id() {
    let at = |id: &str, pos: &str| {
        node(
            id,
            "rect",
            Some("f"),
            json!({ "pos": pos, "w": 10, "h": 10 }),
        )
    };
    let nodes = [
        node("f", "frame", None, json!({ "w": 100, "h": 100 })),
        at("z", "b"),
        at("y", "a"),
        at("x", "b"),
    ];
    let ids: Vec<String> = compute_rects(&nodes, &StubMeasurer)
        .into_iter()
        .map(|(i, _)| i)
        .collect();
    // Position "a" first; the two with position "b" are ordered by id.
    assert_eq!(ids, ["f", "y", "x", "z"]);
}

#[test]
fn siblings_without_a_position_keep_list_order() {
    let nodes = [
        node("f", "frame", None, json!({ "w": 100, "h": 100 })),
        node("b", "rect", Some("f"), json!({})),
        node("a", "rect", Some("f"), json!({})),
    ];
    let ids: Vec<String> = compute_rects(&nodes, &StubMeasurer)
        .into_iter()
        .map(|(i, _)| i)
        .collect();
    assert_eq!(ids, ["f", "b", "a"]);
}

#[test]
fn every_root_is_placed_at_its_own_position() {
    let nodes = [
        node(
            "a",
            "frame",
            None,
            json!({ "x": 1, "y": 2, "w": 10, "h": 10 }),
        ),
        node(
            "b",
            "frame",
            None,
            json!({ "x": 30, "y": 40, "w": 20, "h": 20 }),
        ),
    ];
    assert_eq!(rect_of(&nodes, "a"), r(1.0, 2.0, 10.0, 10.0));
    assert_eq!(rect_of(&nodes, "b"), r(30.0, 40.0, 20.0, 20.0));
}

#[test]
fn an_unreachable_cycle_produces_no_rects_instead_of_looping() {
    let nodes = [
        node("a", "frame", Some("b"), json!({})),
        node("b", "frame", Some("a"), json!({})),
    ];
    assert!(compute_rects(&nodes, &StubMeasurer).is_empty());
}
