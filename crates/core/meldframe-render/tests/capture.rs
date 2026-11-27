//! T13 of the thin UI audit: the world rectangle a capture or a PNG export covers, read from the scene (`PaintedBox::paint_bounds`
//! gives the reach of each box, T16). The numbers are those of `captureRectOf` and `captureBounds` of `packages/web-canvas`.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_layout::Rect;
use meldframe_protocol::Node;
use meldframe_render::{
    HitIndex, capture_bounds, capture_bounds_in, capture_rect, capture_rect_in, resolve,
};
use serde_json::{Value, json};

fn boxes(list: Value) -> Vec<meldframe_render::PaintedBox> {
    resolve(&serde_json::from_value::<Vec<Node>>(list).unwrap())
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}

#[test]
fn a_node_with_nothing_beyond_its_box_is_captured_at_its_world_rectangle() {
    let b = boxes(json!([
        {"id": "page", "type": "frame", "x": 100, "y": 50, "w": 400, "h": 300},
        {"id": "card", "type": "rect", "parentId": "page", "x": 10, "y": 20, "w": 40, "h": 30},
    ]));
    assert_eq!(
        capture_rect(&b, "page"),
        Some(rect(100.0, 50.0, 400.0, 300.0))
    );
    // A child's offset is inside its parent: the world rectangle is the sum.
    assert_eq!(
        capture_rect(&b, "card"),
        Some(rect(110.0, 70.0, 40.0, 30.0))
    );
}

#[test]
fn the_outside_border_and_the_shadow_are_in_the_rectangle() {
    let b = boxes(json!([
        {"id": "a", "type": "rect", "x": 100, "y": 50, "w": 40, "h": 20,
         "border": {"width": 4, "color": "#000000", "position": "outside"}},
        {"id": "b", "type": "rect", "x": 300, "y": 50, "w": 40, "h": 20,
         "shadows": [{"x": 10, "y": 6, "blur": 8, "spread": 2, "color": "#000000"}]},
    ]));
    assert_eq!(capture_rect(&b, "a"), Some(rect(96.0, 46.0, 48.0, 28.0)));
    // The shadow covers the box grown by 2, moved by (10, 6), and its blur reaches 8 past that: 0 on the left, 4 above,
    // 20 on the right and 16 below.
    assert_eq!(capture_rect(&b, "b"), Some(rect(300.0, 46.0, 60.0, 40.0)));
}

#[test]
fn a_node_that_is_not_there_has_no_rectangle() {
    let b = boxes(json!([{"id": "a", "type": "rect", "w": 10, "h": 10}]));
    assert_eq!(capture_rect(&b, "gone"), None);
    assert_eq!(capture_rect(&[], "a"), None);
}

#[test]
fn the_page_is_the_union_of_what_every_box_paints_and_nothing_for_an_empty_scene() {
    let b = boxes(json!([
        {"id": "a", "type": "frame", "x": 0, "y": 0, "w": 100, "h": 100},
        {"id": "b", "type": "frame", "x": 300, "y": 200, "w": 50, "h": 50,
         "border": {"width": 10, "color": "#000000", "position": "outside"}},
    ]));
    assert_eq!(capture_bounds(&b), Some(rect(0.0, 0.0, 360.0, 260.0)));
    assert_eq!(capture_bounds(&[]), None);
}

#[test]
fn what_a_clipping_ancestor_cuts_is_not_in_the_page_but_a_child_of_a_frame_that_does_not_clip_counts()
 {
    let b = boxes(json!([
        {"id": "clip", "type": "frame", "x": 0, "y": 0, "w": 100, "h": 100, "clip": true},
        {"id": "in", "type": "rect", "parentId": "clip", "x": 80, "y": 80, "w": 100, "h": 100},
        {"id": "free", "type": "frame", "x": 200, "y": 0, "w": 100, "h": 100, "clip": false},
        {"id": "out", "type": "rect", "parentId": "free", "x": 90, "y": 90, "w": 100, "h": 100},
    ]));
    // `in` is cut to the clip frame (it ends at 100), `out` reaches to 390 and 190 because its parent does not clip.
    assert_eq!(capture_bounds(&b), Some(rect(0.0, 0.0, 390.0, 190.0)));
}

#[test]
fn a_box_wholly_cut_away_adds_nothing() {
    let b = boxes(json!([
        {"id": "clip", "type": "frame", "x": 0, "y": 0, "w": 100, "h": 100, "clip": true},
        {"id": "away", "type": "rect", "parentId": "clip", "x": 500, "y": 500, "w": 10, "h": 10},
    ]));
    assert_eq!(capture_bounds(&b), Some(rect(0.0, 0.0, 100.0, 100.0)));
}

#[test]
fn a_held_index_gives_the_same_rectangles() {
    let b = boxes(json!([
        {"id": "clip", "type": "frame", "x": 0, "y": 0, "w": 100, "h": 100, "clip": true},
        {"id": "in", "type": "rect", "parentId": "clip", "x": 80, "y": 80, "w": 100, "h": 100,
         "shadows": [{"x": 4, "y": 4, "blur": 6, "spread": 0, "color": "#000000"}]},
    ]));
    let index = HitIndex::from_boxes(&b);
    for id in ["clip", "in", "gone"] {
        assert_eq!(
            capture_rect_in(&index, &b, id),
            capture_rect(&b, id),
            "{id}"
        );
    }
    assert_eq!(capture_bounds_in(&index, &b), capture_bounds(&b));
}
