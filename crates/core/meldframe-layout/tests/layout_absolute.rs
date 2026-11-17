//! E3.2: an absolute child of an auto layout frame (`absolute: true`) leaves the flow: it sits at its own `x` and `y`
//! inside the parent and takes no room, no gap and no share. Every expected number is worked out by hand.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{node, r, rect_of};
use serde_json::json;

fn row(extra: serde_json::Value) -> meldframe_protocol::Node {
    let mut v = json!({"layout": "row", "w": 200, "h": 100, "padding": 0, "gap": 8});
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    node("f", "frame", None, v)
}

#[test]
fn an_absolute_child_sits_at_its_own_offset_inside_the_parent() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({"layout": "row", "x": 10, "y": 20, "w": 200, "h": 100, "padding": 16}),
        ),
        node(
            "p",
            "rect",
            Some("f"),
            json!({"absolute": true, "x": 50, "y": 30, "w": 10, "h": 10}),
        ),
    ];
    // Inside the parent at (10, 20): 10 + 50 and 20 + 30; the padding does not move it.
    assert_eq!(rect_of(&nodes, "p"), r(60.0, 50.0, 10.0, 10.0));
}

#[test]
fn it_takes_no_room_and_no_gap_in_the_flow() {
    let nodes = [
        row(json!({})),
        node("a", "rect", Some("f"), json!({"w": 20, "h": 20})),
        node(
            "p",
            "rect",
            Some("f"),
            json!({"absolute": true, "x": 90, "y": 5, "w": 10, "h": 10}),
        ),
        node("b", "rect", Some("f"), json!({"w": 20, "h": 20})),
    ];
    // a: x = 0. b follows a and one gap of 8: x = 20 + 8 = 28 (not 56).
    assert_eq!(rect_of(&nodes, "a").x, 0.0);
    assert_eq!(rect_of(&nodes, "b").x, 28.0);
    assert_eq!(rect_of(&nodes, "p"), r(90.0, 5.0, 10.0, 10.0));
}

#[test]
fn a_hug_frame_ignores_it() {
    let nodes = [
        row(json!({"sw": "hug", "sh": "hug"})),
        node("a", "rect", Some("f"), json!({"w": 20, "h": 20})),
        node(
            "p",
            "rect",
            Some("f"),
            json!({"absolute": true, "x": 0, "y": 0, "w": 500, "h": 500}),
        ),
        node("b", "rect", Some("f"), json!({"w": 30, "h": 10})),
    ];
    // Width 20 + 8 + 30 = 58, height 20: the 500 x 500 absolute child does not stretch it.
    let f = rect_of(&nodes, "f");
    assert_eq!((f.w, f.h), (58.0, 20.0));
}

#[test]
fn a_fill_child_and_justify_do_not_count_it() {
    let nodes = [
        row(json!({})),
        node("a", "rect", Some("f"), json!({"sw": "fill", "h": 20})),
        node(
            "p",
            "rect",
            Some("f"),
            json!({"absolute": true, "w": 10, "h": 10}),
        ),
        node("b", "rect", Some("f"), json!({"w": 40, "h": 20})),
    ];
    // Two flow children and one gap: a fills 200 - 40 - 8 = 152.
    assert_eq!(rect_of(&nodes, "a").w, 152.0);
    let nodes = [
        row(json!({"justify": "between"})),
        node("a", "rect", Some("f"), json!({"w": 20, "h": 20})),
        node(
            "p",
            "rect",
            Some("f"),
            json!({"absolute": true, "w": 10, "h": 10}),
        ),
        node("b", "rect", Some("f"), json!({"w": 20, "h": 20})),
    ];
    // Between two flow children: b at the far end, x = 200 - 20 = 180.
    assert_eq!(rect_of(&nodes, "b").x, 180.0);
}

#[test]
fn margins_do_not_move_an_absolute_child() {
    let nodes = [
        row(json!({})),
        node(
            "p",
            "rect",
            Some("f"),
            json!({"absolute": true, "x": 5, "y": 6, "w": 10, "h": 10, "marL": 30, "marT": 30}),
        ),
    ];
    assert_eq!(rect_of(&nodes, "p"), r(5.0, 6.0, 10.0, 10.0));
}

#[test]
fn an_absolute_child_keeps_its_own_size_even_when_it_says_fill() {
    let nodes = [
        row(json!({})),
        node(
            "p",
            "rect",
            Some("f"),
            json!({"absolute": true, "sw": "fill", "w": 33, "h": 10}),
        ),
    ];
    assert_eq!(rect_of(&nodes, "p").w, 33.0);
}

#[test]
fn a_node_that_is_not_absolute_lays_out_as_before() {
    let nodes = [
        row(json!({})),
        node("a", "rect", Some("f"), json!({"w": 20, "h": 20})),
    ];
    assert_eq!(rect_of(&nodes, "a"), r(0.0, 0.0, 20.0, 20.0));
}

#[test]
fn the_drawing_order_stays_the_sibling_order() {
    use meldframe_layout::{StubMeasurer, compute_rects};
    // `pos` decides the sibling order: a, then the absolute p, then b.
    let nodes = [
        row(json!({})),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"pos": "a0", "w": 20, "h": 20}),
        ),
        node(
            "p",
            "rect",
            Some("f"),
            json!({"pos": "a1", "absolute": true, "w": 10, "h": 10}),
        ),
        node(
            "b",
            "rect",
            Some("f"),
            json!({"pos": "a2", "w": 20, "h": 20}),
        ),
    ];
    let ids: Vec<String> = compute_rects(&nodes, &StubMeasurer)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids, ["f", "a", "p", "b"]);
}

#[test]
fn absolute_in_a_frame_without_layout_changes_nothing() {
    let nodes = [
        node("f", "frame", None, json!({"w": 200, "h": 100})),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"x": 5, "y": 7, "w": 20, "h": 20, "absolute": true}),
        ),
    ];
    assert_eq!(rect_of(&nodes, "a"), r(5.0, 7.0, 20.0, 20.0));
}

#[test]
fn an_absolute_layout_frame_still_lays_out_its_own_children_and_takes_no_room_in_its_parent() {
    let nodes = [
        row(json!({})),
        node("a", "rect", Some("f"), json!({"w": 20, "h": 20})),
        node(
            "g",
            "frame",
            Some("f"),
            json!({"absolute": true, "layout": "row", "x": 100, "y": 10, "w": 60, "h": 40, "padding": 5}),
        ),
        node("c", "rect", Some("g"), json!({"w": 10, "h": 10})),
        node("b", "rect", Some("f"), json!({"w": 20, "h": 20})),
    ];
    // g sits at (100, 10); its child is laid out in its row: x = 100 + 5, y = 10 + 5. b follows a and one gap.
    assert_eq!(rect_of(&nodes, "g"), r(100.0, 10.0, 60.0, 40.0));
    assert_eq!(rect_of(&nodes, "c"), r(105.0, 15.0, 10.0, 10.0));
    assert_eq!(rect_of(&nodes, "b").x, 28.0);
}
