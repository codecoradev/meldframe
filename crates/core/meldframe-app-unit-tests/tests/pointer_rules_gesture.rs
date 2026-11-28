//! The pointer machine that sizes, draws and sweeps, against the made-up canvas of `pointer_rig`, ported from the resize and create
//! tests of `packages/web-canvas/src/interaction.test.ts` with the same numbers, and the marquee, which the TypeScript had not.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod pointer_rig;
use meldframe_app_model::{
    Camera, CreateKind, Handle, Point, PointerCursor, PointerGesture, PointerPhase, Tool,
};
use meldframe_layout::Rect;
use pointer_rig::*;
use serde_json::json;

fn box_a() -> Rect {
    // 100 by 100 on the screen at zoom 2.
    Rect {
        x: 0.0,
        y: 0.0,
        w: 50.0,
        h: 50.0,
    }
}

fn zoom2() -> Camera {
    Camera {
        x: 0.0,
        y: 0.0,
        zoom: 2.0,
    }
}

fn resize_of(by: (f64, f64), handle: &str) -> serde_json::Value {
    json!({"type": "gesture", "op": {"type": "resizeHandle", "id": "a", "handle": handle,
        "by": {"x": by.0, "y": by.1}, "mods": {"ctrl": false, "meta": false, "alt": false, "shift": false}}})
}

// --- resize --------------------------------------------------------------------------------------------------------

#[test]
fn a_press_on_a_handle_resizes_the_selection_and_selects_nothing() {
    let mut r = rig(zoom2(), &["a"]);
    r.rect = Some(box_a());
    r.down(100.0, 100.0); // the south east handle of the screen box
    r.mv(120.0, 110.0);
    assert_eq!(
        r.machine.view().gesture,
        PointerGesture::Resize {
            id: "a".into(),
            handle: Handle::Se,
            by: Point { x: 10.0, y: 5.0 }
        }
    );
    r.up(120.0, 110.0);
    assert_eq!(r.json(), [resize_of((10.0, 5.0), "se")]);
}

#[test]
fn a_resize_released_back_near_its_start_still_ends_with_the_action() {
    let mut r = rig(Camera::default(), &["a"]);
    r.rect = Some(Rect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    });
    r.down(100.0, 100.0);
    r.mv(120.0, 120.0);
    r.mv(101.0, 100.0); // back inside the drag threshold after the resize began
    r.up(101.0, 100.0);
    assert_eq!(r.json(), [resize_of((1.0, 0.0), "se")]);
}

#[test]
fn a_press_on_a_handle_that_never_leaves_the_threshold_makes_nothing() {
    let mut r = rig(Camera::default(), &["a"]);
    r.rect = Some(Rect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    });
    r.down(100.0, 100.0);
    r.up(101.0, 100.0);
    assert!(r.log.is_empty());
}

#[test]
fn hovering_a_handle_sets_the_resize_cursor_and_the_node_is_not_hovered() {
    let mut r = rig(Camera::default(), &["a"]);
    r.rect = Some(Rect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    });
    r.mv(0.0, 0.0);
    assert_eq!(r.machine.view().cursor, PointerCursor::NwseResize);
    assert_eq!(r.machine.view().hover, None);
    r.mv(100.0, 50.0);
    assert_eq!(r.machine.view().cursor, PointerCursor::EwResize);
    r.mv(50.0, 0.0);
    assert_eq!(r.machine.view().cursor, PointerCursor::NsResize);
    r.mv(100.0, 0.0);
    assert_eq!(r.machine.view().cursor, PointerCursor::NeswResize);
    r.mv(50.0, 50.0);
    assert_eq!(r.machine.view().cursor, PointerCursor::Default);
    assert_eq!(r.machine.view().hover.as_deref(), Some("a"));
}

#[test]
fn the_handles_keep_their_size_in_pixels_at_any_zoom() {
    // At zoom 0.1 the box is 20 pixels across, so only its corners are offered: the middle of an edge is the node, not a handle.
    let mut r = rig(
        Camera {
            x: 0.0,
            y: 0.0,
            zoom: 0.1,
        },
        &["a"],
    );
    r.rect = Some(Rect {
        x: 0.0,
        y: 0.0,
        w: 200.0,
        h: 200.0,
    });
    r.mv(20.0, 10.0);
    assert_eq!(
        r.machine.view().cursor,
        PointerCursor::Default,
        "no edge handle on a small box"
    );
    r.mv(20.0, 20.0);
    assert_eq!(r.machine.view().cursor, PointerCursor::NwseResize);
}

// --- draw ----------------------------------------------------------------------------------------------------------

fn draw_of(kind: &str, from: (f64, f64), to: (f64, f64)) -> serde_json::Value {
    json!({"type": "gesture", "op": {"type": "draw", "kind": kind, "from": {"x": from.0, "y": from.1},
        "to": {"x": to.0, "y": to.1}, "mods": {"ctrl": false, "meta": false, "alt": false, "shift": false}}})
}

#[test]
fn a_drag_with_a_tool_draws_between_the_two_world_points() {
    let mut r = rig(
        Camera {
            x: 100.0,
            y: 0.0,
            zoom: 2.0,
        },
        &[],
    );
    r.tool = Tool::Rectangle;
    r.down(150.0, 100.0); // world (25, 50)
    assert_eq!(r.machine.view().cursor, PointerCursor::Crosshair);
    r.mv(110.0, 60.0); // world (5, 30): dragging up and left
    assert_eq!(
        r.machine.view().gesture,
        PointerGesture::Draw {
            kind: CreateKind::Rectangle,
            from: Point { x: 25.0, y: 50.0 },
            to: Point { x: 5.0, y: 30.0 }
        }
    );
    r.up(110.0, 60.0);
    assert_eq!(r.json(), [draw_of("rectangle", (25.0, 50.0), (5.0, 30.0))]);
}

#[test]
fn every_drawing_tool_names_its_kind_and_a_click_draws_from_a_point_to_itself() {
    for (tool, kind) in [
        (Tool::Frame, "frame"),
        (Tool::Rectangle, "rectangle"),
        (Tool::Ellipse, "ellipse"),
        (Tool::Text, "text"),
    ] {
        let mut r = rig(Camera::default(), &[]);
        r.tool = tool;
        r.down(500.0, 500.0);
        r.up(500.0, 500.0);
        assert_eq!(r.json(), [draw_of(kind, (500.0, 500.0), (500.0, 500.0))]);
    }
}

#[test]
fn a_press_that_wobbles_inside_the_threshold_is_still_a_click() {
    let mut r = rig(Camera::default(), &[]);
    r.tool = Tool::Rectangle;
    r.down(500.0, 500.0);
    r.mv(502.0, 501.0);
    assert_eq!(r.machine.view().gesture, PointerGesture::Idle);
    r.up(502.0, 501.0);
    assert_eq!(
        r.json(),
        [draw_of("rectangle", (500.0, 500.0), (500.0, 500.0))]
    );
}

#[test]
fn a_cancel_while_drawing_makes_no_action() {
    let mut r = rig(Camera::default(), &[]);
    r.tool = Tool::Text;
    r.down(500.0, 500.0);
    r.mv(550.0, 540.0);
    r.send(PointerPhase::Cancel, 550.0, 540.0, false);
    r.up(550.0, 540.0);
    assert!(r.log.is_empty());
}

// --- marquee -------------------------------------------------------------------------------------------------------

#[test]
fn a_marquee_selects_what_it_overlaps_on_release() {
    let mut r = rig(Camera::default(), &[]);
    r.down(500.0, 500.0);
    r.mv(510.0, 500.0);
    assert!(r.log.is_empty());
    r.mv(-10.0, -10.0);
    assert!(matches!(
        r.machine.view().gesture,
        PointerGesture::Marquee { .. }
    ));
    r.up(50.0, 50.0);
    assert_eq!(r.selection, ["a", "b"]);
    assert_eq!(r.json(), [select(&["a", "b"])]);
}

#[test]
fn a_marquee_that_meets_nothing_clears_and_shift_keeps_and_adds() {
    let mut r = rig(Camera::default(), &["a"]);
    r.down(500.0, 500.0);
    r.mv(600.0, 600.0);
    r.up(600.0, 600.0);
    assert_eq!(r.json(), [select(&[])]);
    let mut s = rig(Camera::default(), &["a"]);
    s.send(PointerPhase::Down, 500.0, 500.0, true);
    s.send(PointerPhase::Move, 600.0, 600.0, true);
    s.send(PointerPhase::Up, 600.0, 600.0, true);
    assert!(s.log.is_empty(), "nothing to add");
    s.send(PointerPhase::Down, 150.0, 150.0, true);
    s.send(PointerPhase::Move, 250.0, 50.0, true);
    s.send(PointerPhase::Up, 250.0, 50.0, true);
    assert_eq!(
        s.json(),
        [select(&["a", "b"])],
        "added, in the order it was"
    );
}
