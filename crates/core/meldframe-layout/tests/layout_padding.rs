//! E3.2: padding per side (`padT`, `padR`, `padB`, `padL`). A side that is not set takes the single `padding`, so a
//! file written before the sides existed lays out as before. Every expected number is worked out by hand.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{node, r, rect_of};
use serde_json::json;

#[test]
fn a_row_places_its_child_inside_the_left_and_top_padding() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({"layout": "row", "w": 200, "h": 100, "padding": 0, "padL": 10, "padT": 5, "padR": 30, "padB": 15}),
        ),
        node("c", "rect", Some("f"), json!({"w": 20, "h": 20})),
    ];
    // x = 0 + padL 10, y = 0 + padT 5.
    assert_eq!(rect_of(&nodes, "c"), r(10.0, 5.0, 20.0, 20.0));
}

#[test]
fn a_hug_frame_adds_each_side() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({"layout": "row", "sw": "hug", "sh": "hug", "padding": 0, "padL": 10, "padT": 5, "padR": 30, "padB": 15}),
        ),
        node("c", "rect", Some("f"), json!({"w": 20, "h": 20})),
    ];
    // Width 10 + 20 + 30 = 60; height 5 + 20 + 15 = 40.
    let f = rect_of(&nodes, "f");
    assert_eq!((f.w, f.h), (60.0, 40.0));
}

#[test]
fn a_column_hug_frame_adds_each_side_too() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({"layout": "column", "sw": "hug", "sh": "hug", "padding": 0, "padL": 4, "padT": 8, "padR": 6, "padB": 2}),
        ),
        node("a", "rect", Some("f"), json!({"w": 20, "h": 10})),
        node("b", "rect", Some("f"), json!({"w": 30, "h": 10})),
    ];
    // Gap 8 between the two: width 4 + 30 + 6 = 40; height 8 + 10 + 8 + 10 + 2 = 38.
    let f = rect_of(&nodes, "f");
    assert_eq!((f.w, f.h), (40.0, 38.0));
    // The second child sits below the first: y = 8 + 10 + 8 = 26.
    assert_eq!(rect_of(&nodes, "b").y, 26.0);
}

#[test]
fn a_fill_child_takes_what_the_sides_leave() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({"layout": "row", "w": 200, "h": 100, "padding": 0, "padL": 10, "padR": 30}),
        ),
        node("c", "rect", Some("f"), json!({"sw": "fill", "h": 10})),
    ];
    // 200 - 10 - 30 = 160.
    assert_eq!(rect_of(&nodes, "c").w, 160.0);
}

#[test]
fn centre_alignment_uses_the_space_between_the_sides() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({"layout": "row", "align": "center", "w": 200, "h": 100, "padding": 0, "padT": 10, "padB": 30}),
        ),
        node("c", "rect", Some("f"), json!({"w": 20, "h": 20})),
    ];
    // Inner height 100 - 10 - 30 = 60; the child is centred in it: y = 10 + (60 - 20) / 2 = 30.
    assert_eq!(rect_of(&nodes, "c").y, 30.0);
}

#[test]
fn a_side_that_is_not_set_takes_the_single_padding() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({"layout": "row", "w": 200, "h": 100, "padding": 12, "padL": 3}),
        ),
        node("c", "rect", Some("f"), json!({"w": 20, "h": 20})),
    ];
    // Left 3, top 12 (not set, so `padding`).
    assert_eq!(rect_of(&nodes, "c"), r(3.0, 12.0, 20.0, 20.0));
}

#[test]
fn a_node_without_any_side_lays_out_as_before() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({"layout": "row", "w": 200, "h": 100, "padding": 16}),
        ),
        node("c", "rect", Some("f"), json!({"w": 20, "h": 20})),
    ];
    assert_eq!(rect_of(&nodes, "c"), r(16.0, 16.0, 20.0, 20.0));
}
