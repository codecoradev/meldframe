//! E3.1: `resolve_laid_out` is `resolve_with` with the boxes the layout core gives (hug, fill, auto layout), each
//! offset still relative to its parent, as the canvas reads it.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_layout::{StubMeasurer, compute_rects};
use meldframe_protocol::{Node, Variables};
use meldframe_render::{ModeChoice, PaintedBox, resolve, resolve_laid_out, resolve_with};
use serde_json::{Value, json};
use std::collections::HashMap;

const FIXTURES: &str = include_str!("fixtures/scene.json");

fn nodes(list: Value) -> Vec<Node> {
    serde_json::from_value(list).unwrap()
}

fn laid_out(nodes: &[Node]) -> Vec<PaintedBox> {
    resolve_laid_out(
        nodes,
        &Variables::default(),
        &ModeChoice::default(),
        &StubMeasurer,
    )
}

fn at<'a>(boxes: &'a [PaintedBox], id: &str) -> &'a PaintedBox {
    boxes
        .iter()
        .find(|b| b.id == id)
        .unwrap_or_else(|| panic!("no box for {id}"))
}

fn geometry(b: &PaintedBox) -> (f64, f64, f64, f64) {
    (b.x, b.y, b.w, b.h)
}

#[test]
fn a_file_with_no_layout_paints_exactly_what_resolve_paints() {
    let cases: Vec<Value> = serde_json::from_str(FIXTURES).unwrap();
    assert!(cases.len() >= 8);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let list: Vec<Node> = serde_json::from_value(case["nodes"].clone()).unwrap();
        assert_eq!(laid_out(&list), resolve(&list), "{name}");
    }
}

#[test]
fn a_fractional_offset_in_a_free_frame_is_kept_to_the_bit() {
    // 100.7 + 10.3 - 100.7 is not 10.3 in floating point; the offset the person typed must come back as it was.
    let list = nodes(json!([
        {"id": "p", "type": "frame", "x": 100.7, "y": 0.1, "w": 300, "h": 300},
        {"id": "c", "type": "rect", "parentId": "p", "x": 10.3, "y": 0.2, "w": 20, "h": 20},
    ]));
    let boxes = laid_out(&list);
    assert_eq!(geometry(at(&boxes, "c")), (10.3, 0.2, 20.0, 20.0));
}

#[test]
fn the_children_of_a_column_are_stacked_inside_its_padding_and_gap() {
    // Stored offsets are all 0, as an agent that left them out would write them.
    let list = nodes(json!([
        {"id": "p", "type": "frame", "x": 50, "y": 60, "w": 100, "h": 200, "layout": "column", "padding": 10, "gap": 4},
        {"id": "a", "type": "rect", "parentId": "p", "pos": "a", "w": 50, "h": 20},
        {"id": "b", "type": "rect", "parentId": "p", "pos": "b", "w": 50, "h": 30},
    ]));
    let boxes = laid_out(&list);
    assert_eq!(geometry(at(&boxes, "p")), (50.0, 60.0, 100.0, 200.0));
    assert_eq!(geometry(at(&boxes, "a")), (10.0, 10.0, 50.0, 20.0));
    assert_eq!(geometry(at(&boxes, "b")), (10.0, 34.0, 50.0, 30.0));
}

#[test]
fn a_row_shares_what_is_left_among_the_children_that_fill() {
    let list = nodes(json!([
        {"id": "p", "type": "frame", "w": 200, "h": 40, "layout": "row", "gap": 10, "padding": 0},
        {"id": "a", "type": "rect", "parentId": "p", "pos": "a", "w": 50, "h": 40},
        {"id": "b", "type": "rect", "parentId": "p", "pos": "b", "w": 1, "h": 40, "sw": "fill"},
        {"id": "c", "type": "rect", "parentId": "p", "pos": "c", "w": 1, "h": 40, "sw": "fill"},
    ]));
    let boxes = laid_out(&list);
    // 200 - 50 - 2 gaps of 10 = 130, shared by two.
    assert_eq!(geometry(at(&boxes, "b")), (60.0, 0.0, 65.0, 40.0));
    assert_eq!(geometry(at(&boxes, "c")), (135.0, 0.0, 65.0, 40.0));
}

#[test]
fn a_hug_frame_and_a_hug_text_take_the_size_of_what_is_inside() {
    let list = nodes(json!([
        {"id": "p", "type": "frame", "w": 1, "h": 1, "sw": "hug", "sh": "hug", "layout": "column", "padding": 8},
        {"id": "t", "type": "text", "parentId": "p", "text": "hello", "fontSize": 16, "sw": "hug", "sh": "hug"},
    ]));
    let boxes = laid_out(&list);
    // The stub measures "hello" at 16 px as 40 x 20.
    assert_eq!(geometry(at(&boxes, "t")), (8.0, 8.0, 40.0, 20.0));
    assert_eq!(geometry(at(&boxes, "p")), (0.0, 0.0, 56.0, 36.0));
}

#[test]
fn a_nested_frame_is_placed_by_its_parent_and_its_children_stay_relative_to_it() {
    let list = nodes(json!([
        {"id": "page", "type": "frame", "x": 10, "y": 10, "w": 300, "h": 300, "layout": "column", "padding": 20},
        {"id": "box", "type": "frame", "parentId": "page", "w": 100, "h": 100},
        {"id": "dot", "type": "rect", "parentId": "box", "x": 5, "y": 6, "w": 10, "h": 10},
    ]));
    let boxes = laid_out(&list);
    assert_eq!(geometry(at(&boxes, "box")), (20.0, 20.0, 100.0, 100.0));
    assert_eq!(geometry(at(&boxes, "dot")), (5.0, 6.0, 10.0, 10.0));
}

#[test]
fn an_absolute_child_keeps_its_own_offset_and_takes_no_room() {
    let list = nodes(json!([
        {"id": "p", "type": "frame", "w": 100, "h": 100, "layout": "column", "padding": 10},
        {"id": "badge", "type": "rect", "parentId": "p", "pos": "a", "x": 70, "y": 3, "w": 20, "h": 20, "absolute": true},
        {"id": "a", "type": "rect", "parentId": "p", "pos": "b", "w": 30, "h": 30},
    ]));
    let boxes = laid_out(&list);
    assert_eq!(geometry(at(&boxes, "badge")), (70.0, 3.0, 20.0, 20.0));
    assert_eq!(geometry(at(&boxes, "a")), (10.0, 10.0, 30.0, 30.0));
}

#[test]
fn an_ellipse_is_round_at_the_size_the_layout_gives_it() {
    let list = nodes(json!([
        {"id": "p", "type": "frame", "w": 200, "h": 60, "layout": "row", "padding": 0},
        {"id": "e", "type": "ellipse", "parentId": "p", "w": 10, "h": 60, "sw": "fill"},
    ]));
    let boxes = laid_out(&list);
    assert_eq!(geometry(at(&boxes, "e")), (0.0, 0.0, 200.0, 60.0));
    assert_eq!(at(&boxes, "e").radius, 30.0);
}

#[test]
fn only_the_geometry_differs_from_resolve_and_the_order_is_the_same() {
    let list = nodes(json!([
        {"id": "p", "type": "frame", "w": 100, "h": 100, "layout": "row", "padding": 5, "fill": "#102030", "clip": false},
        {"id": "z", "type": "rect", "parentId": "p", "pos": "b", "w": 10, "h": 10, "fill": "#ff0000", "opacity": 0.5},
        {"id": "y", "type": "text", "parentId": "p", "pos": "a", "text": "hi", "fontSize": 12, "sw": "hug"},
    ]));
    let (plain, laid) = (resolve(&list), laid_out(&list));
    assert_eq!(
        plain.iter().map(|b| &b.id).collect::<Vec<_>>(),
        laid.iter().map(|b| &b.id).collect::<Vec<_>>()
    );
    for (a, b) in plain.into_iter().zip(laid) {
        let strip = |mut b: PaintedBox| {
            (b.x, b.y, b.w, b.h) = (0.0, 0.0, 0.0, 0.0);
            b
        };
        assert_eq!(strip(a), strip(b));
    }
}

#[test]
fn bound_fields_paint_the_value_of_their_mode_as_resolve_with_does() {
    let list = nodes(json!([
        {"id": "p", "type": "frame", "w": 100, "h": 100, "layout": "column", "fill": {"var": "missing"}},
    ]));
    let vars = Variables::default();
    let choice = ModeChoice::default();
    let a = resolve_laid_out(&list, &vars, &choice, &StubMeasurer);
    let b = resolve_with(&list, &vars, &choice);
    assert_eq!(a[0].fill, b[0].fill);
}

#[test]
fn a_frame_whose_parent_is_missing_is_a_root_and_is_laid_out_as_one() {
    let list = nodes(json!([
        {"id": "o", "type": "frame", "parentId": "gone", "x": 5, "y": 6, "w": 100, "h": 100, "layout": "column", "padding": 10},
        {"id": "c", "type": "rect", "parentId": "o", "w": 20, "h": 20},
    ]));
    let boxes = laid_out(&list);
    assert_eq!(at(&boxes, "o").parent, None);
    assert_eq!(geometry(at(&boxes, "o")), (5.0, 6.0, 100.0, 100.0));
    assert_eq!(geometry(at(&boxes, "c")), (10.0, 10.0, 20.0, 20.0));
}

#[test]
fn a_child_of_a_node_that_is_not_a_frame_keeps_its_stored_box() {
    // The layout core places the children of frames only; the rest of the file still shows.
    let list = nodes(json!([
        {"id": "p", "type": "frame", "w": 300, "h": 300, "layout": "column", "padding": 40},
        {"id": "r", "type": "rect", "parentId": "p", "w": 100, "h": 100},
        {"id": "k", "type": "rect", "parentId": "r", "x": 7, "y": 8, "w": 9, "h": 10},
    ]));
    let boxes = laid_out(&list);
    assert_eq!(geometry(at(&boxes, "r")), (40.0, 40.0, 100.0, 100.0));
    assert_eq!(geometry(at(&boxes, "k")), (7.0, 8.0, 9.0, 10.0));
}

#[test]
fn a_cycle_stays_visible_at_its_stored_boxes() {
    let list = nodes(json!([
        {"id": "a", "type": "frame", "parentId": "b", "x": 1, "y": 2, "w": 30, "h": 30, "layout": "row"},
        {"id": "b", "type": "frame", "parentId": "a", "x": 3, "y": 4, "w": 40, "h": 40},
    ]));
    let boxes = laid_out(&list);
    assert_eq!(boxes.len(), 2);
    assert_eq!(geometry(at(&boxes, "a")), (1.0, 2.0, 30.0, 30.0));
    assert_eq!(geometry(at(&boxes, "b")), (3.0, 4.0, 40.0, 40.0));
}

/// Adds the offsets of the parents up: the box on the canvas itself.
fn world_rects(boxes: &[PaintedBox]) -> HashMap<&str, (f64, f64, f64, f64)> {
    let mut out: HashMap<&str, (f64, f64, f64, f64)> = HashMap::new();
    for b in boxes {
        let (ox, oy) = b
            .parent
            .as_deref()
            .map_or((0.0, 0.0), |p| (out[p].0, out[p].1));
        out.insert(&b.id, (ox + b.x, oy + b.y, b.w, b.h));
    }
    out
}

#[test]
fn the_boxes_add_up_to_the_rects_of_the_layout_core() {
    let list = nodes(json!([
        {"id": "page", "type": "frame", "x": 12.5, "y": 7.25, "w": 400, "h": 500, "layout": "column", "padding": 16, "gap": 12, "align": "center"},
        {"id": "row", "type": "frame", "parentId": "page", "pos": "a", "w": 1, "h": 1, "sw": "fill", "sh": "hug", "layout": "row", "gap": 8, "padT": 4},
        {"id": "t1", "type": "text", "parentId": "row", "pos": "a", "text": "Meld frame", "fontSize": 14, "sw": "hug", "sh": "hug"},
        {"id": "r1", "type": "rect", "parentId": "row", "pos": "b", "w": 1, "h": 30, "sw": "fill", "marL": 3},
        {"id": "card", "type": "frame", "parentId": "page", "pos": "b", "w": 150, "h": 90, "layout": "column", "justify": "between", "padding": 6},
        {"id": "e", "type": "ellipse", "parentId": "card", "pos": "a", "w": 20, "h": 20},
        {"id": "f", "type": "rect", "parentId": "card", "pos": "b", "w": 20, "h": 20},
        {"id": "pin", "type": "rect", "parentId": "card", "pos": "c", "x": 120, "y": 2, "w": 10, "h": 10, "absolute": true},
    ]));
    let boxes = laid_out(&list);
    let world = world_rects(&boxes);
    let rects = compute_rects(&list, &StubMeasurer);
    assert_eq!(rects.len(), list.len());
    for (id, r) in rects {
        let (x, y, w, h) = world[id.as_str()];
        assert!(
            (x - r.x).abs() < 1e-9 && (y - r.y).abs() < 1e-9 && w == r.w && h == r.h,
            "{id}: boxes say ({x}, {y}, {w}, {h}), layout says {r:?}"
        );
    }
}
