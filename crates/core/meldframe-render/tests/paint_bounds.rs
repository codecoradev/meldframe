//! T16 of the thin UI audit: how far a box paints beyond its layout box (`PaintedBox::paint_bounds`) is worked out once,
//! here, and the canvas's capture box reads it. The expected rectangles are the numbers the TypeScript reach of
//! `packages/web-paint` (`borderReach`, `shadowBounds`) and the capture tests of `packages/web-canvas` gave before the
//! reach moved, so a change of a number is a change of what an agent's capture covers.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_layout::StubMeasurer;
use meldframe_protocol::{Node, Variables};
use meldframe_render::{ModeChoice, PaintedBox, resolve, resolve_laid_out};
use serde_json::{Value, json};

/// The largest blur, offset and spread that count for the reach (`MAX_BLUR` of `web-paint`, which sets the same cap on the
/// blur the canvas draws).
const CAP: f64 = 1024.0;

fn nodes(list: Value) -> Vec<Node> {
    serde_json::from_value(list).unwrap()
}

/// The box of a root node `{x, y, w, h}` with the extra fields, as boxes.
fn boxes_of(extra: Value) -> Vec<PaintedBox> {
    let mut n = json!({"id": "n", "type": "rect", "x": 100, "y": 50, "w": 40, "h": 20});
    n.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    resolve(&nodes(json!([n])))
}

/// `(x, y, w, h)` of the rectangle a box paints into, with the box at `(bx, by)`.
fn painted(b: &PaintedBox, bx: f64, by: f64) -> (f64, f64, f64, f64) {
    let p = b.paint_bounds;
    (
        bx - p.left,
        by - p.top,
        b.w + p.left + p.right,
        b.h + p.top + p.bottom,
    )
}

/// The painted rectangle of the one box of `extra`, which sits at 100, 50 and is 40 by 20.
fn rect_of(extra: Value) -> (f64, f64, f64, f64) {
    painted(&boxes_of(extra)[0], 100.0, 50.0)
}

fn shadow(x: f64, y: f64, blur: f64, spread: f64) -> Value {
    json!({"x": x, "y": y, "blur": blur, "spread": spread, "color": "#000000"})
}

const BOX: (f64, f64, f64, f64) = (100.0, 50.0, 40.0, 20.0);

#[test]
fn a_box_with_no_border_and_no_shadow_paints_into_its_own_box() {
    assert_eq!(rect_of(json!({})), BOX);
    assert!(boxes_of(json!({}))[0].paint_bounds.is_flush());
}

#[test]
fn a_shadow_reaches_by_its_offset_spread_and_blur() {
    // `shadowBounds` of web-paint: y 4, blur 8, spread 2.
    assert_eq!(
        rect_of(json!({"shadows": [shadow(0.0, 4.0, 8.0, 2.0)]})),
        (90.0, 44.0, 60.0, 40.0)
    );
}

#[test]
fn the_reach_of_several_shadows_is_the_rectangle_around_all_of_them() {
    assert_eq!(
        rect_of(json!({"shadows": [shadow(10.0, 0.0, 0.0, 0.0), shadow(-10.0, 5.0, 3.0, 0.0)]})),
        (87.0, 50.0, 63.0, 28.0)
    );
}

#[test]
fn a_shadow_with_nothing_left_of_it_or_with_no_alpha_has_no_reach() {
    assert_eq!(
        rect_of(json!({"shadows": [shadow(0.0, 0.0, 20.0, -30.0)]})),
        BOX
    );
    let clear = json!({"x": 0, "y": 0, "blur": 30, "spread": 5, "color": "#00000000"});
    assert_eq!(rect_of(json!({"shadows": [clear]})), BOX);
    let clear_far = json!({"x": 500, "y": 0, "blur": 0, "spread": 0, "color": "#00000000"});
    assert_eq!(
        rect_of(json!({"shadows": [clear_far, shadow(0.0, 4.0, 2.0, 0.0)]})),
        // `shadowBounds` gave 98, 52, 44, 24, the shadow alone; the capture box joins it to the box (`paintedRect`).
        (98.0, 50.0, 44.0, 26.0)
    );
}

#[test]
fn a_negative_spread_shrinks_the_shadow_and_the_box_stays_in_its_own_bounds() {
    // The model has drop shadows only: a negative spread draws a smaller shadow, which can lie wholly inside the box. The
    // reach is then the box itself, and a shrunk shadow that is moved out reaches out.
    assert_eq!(
        rect_of(json!({"shadows": [shadow(0.0, 0.0, 0.0, -5.0)]})),
        BOX
    );
    assert_eq!(
        rect_of(json!({"shadows": [shadow(30.0, 0.0, 0.0, -5.0)]})),
        (100.0, 50.0, 40.0 + 25.0, 20.0)
    );
}

#[test]
fn an_offset_and_a_spread_count_for_at_most_the_cap_and_so_does_a_blur() {
    assert_eq!(
        rect_of(json!({"shadows": [shadow(1e6, -1e6, 0.0, 1e6)]})),
        (100.0, 50.0 - 2.0 * CAP, 40.0 + 2.0 * CAP, 20.0 + 2.0 * CAP)
    );
    assert_eq!(
        rect_of(json!({"shadows": [shadow(-1e6, 0.0, 0.0, 0.0)]})),
        (100.0 - CAP, 50.0, 40.0 + CAP, 20.0)
    );
    assert_eq!(
        rect_of(json!({"shadows": [shadow(0.0, 0.0, 0.0, -1e6)]})),
        BOX
    );
    assert_eq!(
        rect_of(json!({"shadows": [shadow(0.0, 0.0, 100_000.0, 0.0)]})),
        (100.0 - CAP, 50.0 - CAP, 40.0 + 2.0 * CAP, 20.0 + 2.0 * CAP)
    );
}

#[test]
fn a_spread_that_takes_a_whole_side_leaves_no_shadow_even_when_it_is_moved_out() {
    // 40 by 20 box. A spread of -30 leaves nothing on either side, and one of -10 leaves 20 by 0: a shadow with no area paints
    // nothing, wherever the offset puts it. Without the skip these would reach 70 and 90 to the right.
    assert_eq!(
        rect_of(json!({"shadows": [shadow(100.0, 0.0, 0.0, -30.0)]})),
        BOX
    );
    assert_eq!(
        rect_of(json!({"shadows": [shadow(100.0, 0.0, 0.0, -10.0)]})),
        BOX
    );
    // A spread of -9 leaves 22 by 2: that one paints and reaches (x 109 to 131 in the box's own terms, 91 past its right edge at 40).
    assert_eq!(
        rect_of(json!({"shadows": [shadow(100.0, 0.0, 0.0, -9.0)]})),
        (100.0, 50.0, 40.0 + 91.0, 20.0)
    );
}

#[test]
fn a_blur_below_zero_is_a_hard_edge() {
    // The model refuses it; a bad file still gives the box the reach of a hard shadow.
    assert_eq!(
        rect_of(json!({"shadows": [shadow(3.0, 0.0, -5.0, 0.0)]})),
        (100.0, 50.0, 43.0, 20.0)
    );
}

fn border(width: f64, position: &str) -> Value {
    json!({"width": width, "color": "#ff0000", "position": position})
}

#[test]
fn a_border_reaches_by_the_share_of_its_width_that_lies_outside() {
    // `borderReach` of web-paint: all of the width outside, half of it centred, none inside.
    assert_eq!(rect_of(json!({"border": border(6.0, "inside")})), BOX);
    assert_eq!(
        rect_of(json!({"border": border(6.0, "center")})),
        (97.0, 47.0, 46.0, 26.0)
    );
    assert_eq!(
        rect_of(json!({"border": border(6.0, "outside")})),
        (94.0, 44.0, 52.0, 32.0)
    );
}

#[test]
fn a_border_and_a_shadow_join_into_one_rectangle() {
    // The `capture box` tests of web-canvas: an outside border of 6 and a shadow moved 20 right and spread by 2.
    let b = &boxes_of(json!({
        "x": 300, "y": 100, "w": 50, "h": 40,
        "border": border(6.0, "outside"),
        "shadows": [shadow(20.0, 0.0, 0.0, 2.0)]
    }))[0];
    assert_eq!(painted(b, 300.0, 100.0), (294.0, 94.0, 78.0, 52.0));
    let a = &boxes_of(json!({
        "x": 100, "y": 100, "w": 50, "h": 40,
        "shadows": [shadow(0.0, 4.0, 10.0, 0.0)]
    }))[0];
    assert_eq!(painted(a, 100.0, 100.0), (90.0, 94.0, 70.0, 60.0));
}

#[test]
fn the_reach_is_the_largest_of_the_border_and_the_shadow_on_each_side() {
    // A border of 6 outside is wider than a shadow that reaches 0 on the left and 5 above and below, and shorter than one that
    // reaches 10 on the right.
    assert_eq!(
        rect_of(json!({
            "border": border(6.0, "outside"),
            "shadows": [shadow(5.0, 0.0, 5.0, 0.0)]
        })),
        (94.0, 44.0, 40.0 + 6.0 + 10.0, 32.0)
    );
}

#[test]
fn a_text_paints_no_border_and_no_shadow_so_it_has_no_reach() {
    let list = nodes(json!([{
        "id": "t", "type": "text", "text": "Hi", "fontSize": 12, "x": 1, "y": 2, "w": 30, "h": 10,
        "border": border(6.0, "outside"), "shadows": [shadow(5.0, 5.0, 5.0, 5.0)]
    }]));
    assert!(resolve(&list)[0].paint_bounds.is_flush());
}

#[test]
fn a_frame_that_clips_does_not_clip_its_own_border_or_shadow() {
    let b = &boxes_of(json!({
        "type": "frame", "clip": true,
        "border": border(4.0, "outside"),
        "shadows": [shadow(0.0, 10.0, 0.0, 0.0)]
    }))[0];
    assert_eq!(painted(b, 100.0, 50.0), (96.0, 46.0, 48.0, 34.0));
}

#[test]
fn the_reach_of_a_nested_box_is_around_its_own_box_whatever_its_parent_is() {
    let list = nodes(json!([
        {"id": "p", "type": "frame", "x": 800, "y": 100, "w": 100, "h": 100, "clip": true},
        {"id": "k", "type": "rect", "parentId": "p", "x": 90, "y": 0, "w": 20, "h": 20,
         "shadows": [shadow(30.0, 0.0, 0.0, 0.0)]},
    ]));
    let boxes = resolve(&list);
    assert!(boxes[0].paint_bounds.is_flush());
    // The offset in the parent is 90, 0: the room around the box is in the box's own terms.
    assert_eq!(painted(&boxes[1], 90.0, 0.0), (90.0, 0.0, 50.0, 20.0));
}

#[test]
fn an_ellipse_reaches_as_the_box_it_is_drawn_in() {
    let b = &boxes_of(json!({
        "type": "ellipse", "border": border(6.0, "outside"),
        "shadows": [shadow(0.0, 0.0, 0.0, 10.0)]
    }))[0];
    assert_eq!(painted(b, 100.0, 50.0), (90.0, 40.0, 60.0, 40.0));
}

#[test]
fn a_box_the_layout_resizes_gets_the_reach_of_its_laid_out_size() {
    // A shadow moved 100 right and spread by -30 has nothing left of a 40 by 20 box, and 240 by 240 of a box that
    // fills a frame of 300 by 300: it ends at 130 + 240 = 370, 70 past the box.
    let list = nodes(json!([
        {"id": "f", "type": "frame", "w": 300, "h": 300, "layout": "row", "padding": 0},
        {"id": "c", "type": "rect", "parentId": "f", "w": 40, "h": 20, "sw": "fill", "sh": "fill",
         "shadows": [shadow(100.0, 0.0, 0.0, -30.0)]},
    ]));
    let stored = &resolve(&list)[1];
    assert!(stored.paint_bounds.is_flush(), "{stored:?}");
    let laid = resolve_laid_out(
        &list,
        &Variables::default(),
        &ModeChoice::default(),
        &StubMeasurer,
    );
    let l = &laid[1];
    assert_eq!((l.w, l.h), (300.0, 300.0));
    assert_eq!(painted(l, 0.0, 0.0), (0.0, 0.0, 370.0, 300.0));
}
