//! E3.2: margin per side (`marT`, `marR`, `marB`, `marL`) of a child in an auto layout frame. A margin is room around
//! the child: it moves the child, takes part in the main-axis total, in hug sizes and in alignment, and is ignored by
//! a frame without layout. Every expected number is worked out by hand.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{node, r, rect_of};
use serde_json::json;

fn row(extra: serde_json::Value) -> meldframe_protocol::Node {
    let mut v = json!({"layout": "row", "w": 200, "h": 100, "padding": 0, "gap": 0});
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    node("f", "frame", None, v)
}

#[test]
fn a_row_adds_the_margins_of_each_child_along_the_main_axis() {
    let nodes = [
        row(json!({})),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"w": 20, "h": 20, "marL": 5, "marR": 10}),
        ),
        node("b", "rect", Some("f"), json!({"w": 20, "h": 20})),
    ];
    // a: x = 5. b: x = 5 + 20 + 10 = 35.
    assert_eq!(rect_of(&nodes, "a").x, 5.0);
    assert_eq!(rect_of(&nodes, "b").x, 35.0);
}

#[test]
fn a_column_adds_the_vertical_margins() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({"layout": "column", "w": 100, "h": 200, "padding": 0, "gap": 0}),
        ),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"w": 20, "h": 20, "marT": 4, "marB": 6}),
        ),
        node("b", "rect", Some("f"), json!({"w": 20, "h": 20})),
    ];
    // a: y = 4. b: y = 4 + 20 + 6 = 30.
    assert_eq!(rect_of(&nodes, "a").y, 4.0);
    assert_eq!(rect_of(&nodes, "b").y, 30.0);
}

#[test]
fn the_cross_axis_margin_moves_the_child_off_the_start_edge() {
    let nodes = [
        row(json!({})),
        node("a", "rect", Some("f"), json!({"w": 20, "h": 20, "marT": 7})),
    ];
    assert_eq!(rect_of(&nodes, "a"), r(0.0, 7.0, 20.0, 20.0));
}

#[test]
fn centre_alignment_centres_the_child_with_its_margins() {
    let nodes = [
        row(json!({"align": "center"})),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"w": 20, "h": 20, "marT": 10}),
        ),
    ];
    // Footprint 20 + 10 = 30 in a height of 100: (100 - 30) / 2 = 35, then the top margin: 45.
    assert_eq!(rect_of(&nodes, "a").y, 45.0);
}

#[test]
fn end_alignment_leaves_the_far_margin_at_the_edge() {
    let nodes = [
        row(json!({"align": "end"})),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"w": 20, "h": 20, "marT": 3, "marB": 9}),
        ),
    ];
    // Footprint 32; end: 100 - 32 = 68, plus marT 3 = 71; the child ends at 91, 9 above the bottom edge.
    assert_eq!(rect_of(&nodes, "a").y, 71.0);
}

#[test]
fn a_fill_child_takes_what_the_other_footprints_and_its_own_margins_leave() {
    let nodes = [
        row(json!({})),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"w": 20, "h": 20, "marL": 5, "marR": 5}),
        ),
        node(
            "b",
            "rect",
            Some("f"),
            json!({"sw": "fill", "h": 20, "marL": 10, "marR": 10}),
        ),
    ];
    // a takes 30; b's margins take 20; b gets 200 - 30 - 20 = 150, at x = 30 + 10 = 40.
    let b = rect_of(&nodes, "b");
    assert_eq!((b.x, b.w), (40.0, 150.0));
}

#[test]
fn a_cross_fill_child_gives_up_its_cross_margins() {
    let nodes = [
        row(json!({})),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"w": 20, "sh": "fill", "marT": 10, "marB": 30}),
        ),
    ];
    // 100 - 10 - 30 = 60, at y = 10.
    let a = rect_of(&nodes, "a");
    assert_eq!((a.y, a.h), (10.0, 60.0));
}

#[test]
fn justify_spreads_what_is_left_after_the_margins() {
    let nodes = [
        row(json!({"justify": "end"})),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"w": 20, "h": 20, "marL": 5, "marR": 15}),
        ),
    ];
    // Footprint 40; end: 200 - 40 = 160, plus marL 5: x = 165, ending 15 from the right edge.
    assert_eq!(rect_of(&nodes, "a").x, 165.0);
}

#[test]
fn a_hug_row_adds_margins_to_its_width_and_the_tallest_footprint_to_its_height() {
    let nodes = [
        row(json!({"sw": "hug", "sh": "hug"})),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"w": 20, "h": 20, "marL": 5, "marR": 5, "marT": 3, "marB": 2}),
        ),
        node("b", "rect", Some("f"), json!({"w": 20, "h": 10})),
    ];
    // Width 5 + 20 + 5 + 20 = 50; height max(20 + 3 + 2, 10) = 25.
    let f = rect_of(&nodes, "f");
    assert_eq!((f.w, f.h), (50.0, 25.0));
}

#[test]
fn a_hug_column_adds_margins_to_its_height_and_the_widest_footprint_to_its_width() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({"layout": "column", "sw": "hug", "sh": "hug", "padding": 0, "gap": 8}),
        ),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"w": 20, "h": 10, "marT": 2, "marB": 3, "marL": 4}),
        ),
        node("b", "rect", Some("f"), json!({"w": 20, "h": 10})),
    ];
    // Height 2 + 10 + 3 + gap 8 + 10 = 33; width max(4 + 20, 20) = 24.
    let f = rect_of(&nodes, "f");
    assert_eq!((f.w, f.h), (24.0, 33.0));
}

#[test]
fn a_frame_without_layout_ignores_margins() {
    let nodes = [
        node("f", "frame", None, json!({"w": 200, "h": 100})),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"x": 5, "y": 7, "w": 20, "h": 20, "marL": 50, "marT": 50}),
        ),
    ];
    assert_eq!(rect_of(&nodes, "a"), r(5.0, 7.0, 20.0, 20.0));
}

#[test]
fn a_node_without_margins_lays_out_as_before() {
    let nodes = [
        row(json!({})),
        node("a", "rect", Some("f"), json!({"w": 20, "h": 20})),
    ];
    assert_eq!(rect_of(&nodes, "a"), r(0.0, 0.0, 20.0, 20.0));
}
