//! The overlay that does not scale with the zoom (E8.1.4, part 1), ported from `packages/web-canvas/src/overlay.test.ts` (the
//! selection box, the handles and the drop slot line) with the same cases and numbers.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app_model::camera::{Rect, rect_to_screen};
use meldframe_app_model::{Camera, Handle, Point};
use meldframe_app_rules::canvas_overlay::*;

fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}
fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

#[test]
fn the_box_is_the_world_rect_through_the_camera_and_the_handles_do_not_scale() {
    let rect = r(100.0, 50.0, 200.0, 80.0);
    for zoom in [0.02, 0.5, 1.0, 4.0, 64.0] {
        let cam = Camera {
            x: 30.0,
            y: -10.0,
            zoom,
        };
        assert_eq!(
            selection_box(&cam, &[rect]),
            Some(rect_to_screen(&cam, &rect))
        );
    }
}

#[test]
fn a_length_in_screen_pixels_is_that_many_over_the_zoom_in_the_world() {
    // the handle is 8 px on screen at any zoom, and so 8 / zoom in the world
    let at = |zoom| {
        world_len(
            HANDLE_SIZE,
            &Camera {
                x: 30.0,
                y: -10.0,
                zoom,
            },
        )
    };
    for (zoom, want) in [
        (0.02, 400.0),
        (0.5, 16.0),
        (1.0, 8.0),
        (4.0, 2.0),
        (64.0, 0.125),
    ] {
        assert_eq!(at(zoom), want, "zoom {zoom}");
    }
    // the reach of the pointer around a handle: half the handle and the slop
    assert_eq!(
        (HANDLE_SIZE, HANDLE_SLOP, HANDLE_MIN_SIDE),
        (8.0, 2.0, 24.0)
    );
}

#[test]
fn several_rectangles_give_their_union_and_none_gives_none() {
    let cam = Camera {
        x: 0.0,
        y: 0.0,
        zoom: 2.0,
    };
    let rect = r(100.0, 50.0, 200.0, 80.0);
    assert_eq!(
        selection_box(&cam, &[rect, r(0.0, 0.0, 10.0, 10.0)]),
        Some(r(0.0, 0.0, 600.0, 260.0))
    );
    assert_eq!(selection_box(&Camera::default(), &[]), None);
}

#[test]
fn eight_handles_sit_on_the_corners_and_edge_midpoints() {
    let b = r(10.0, 20.0, 100.0, 60.0);
    assert_eq!(
        HANDLES.map(|h| handle_position(&b, h)),
        [
            p(10.0, 20.0),
            p(60.0, 20.0),
            p(110.0, 20.0),
            p(110.0, 50.0),
            p(110.0, 80.0),
            p(60.0, 80.0),
            p(10.0, 80.0),
            p(10.0, 50.0)
        ]
    );
    // at several zooms the corners are the scaled corners of the world rect
    for zoom in [0.25, 1.0, 3.0] {
        let cam = Camera {
            x: 17.0,
            y: 9.0,
            zoom,
        };
        let b = selection_box(&cam, &[r(100.0, 50.0, 200.0, 80.0)]).expect("a valid value");
        let (nw, se) = (
            handle_position(&b, Handle::Nw),
            handle_position(&b, Handle::Se),
        );
        assert_eq!(nw, p(100.0 * zoom + 17.0, 50.0 * zoom + 9.0));
        assert_eq!(se, p(300.0 * zoom + 17.0, 130.0 * zoom + 9.0));
    }
}

#[test]
fn small_boxes_drop_the_edge_handles_that_would_touch_each_other() {
    let corners = vec![Handle::Nw, Handle::Ne, Handle::Se, Handle::Sw];
    assert_eq!(visible_handles(&r(0.0, 0.0, 200.0, 100.0)).len(), 8);
    assert_eq!(visible_handles(&r(0.0, 0.0, 10.0, 100.0)), corners);
    assert_eq!(visible_handles(&r(0.0, 0.0, 100.0, 10.0)), corners);
    assert_eq!(visible_handles(&r(0.0, 0.0, 1.0, 1.0)), corners);
}

#[test]
fn handle_at_finds_the_handle_under_the_pointer_and_none_far_away() {
    let b = r(10.0, 20.0, 100.0, 60.0);
    assert_eq!(handle_at(&b, 12.0, 22.0), Some(Handle::Nw));
    assert_eq!(
        handle_at(&b, 60.0, 20.0 - HANDLE_SIZE / 2.0),
        Some(Handle::N)
    );
    assert_eq!(handle_at(&b, 60.0, 50.0), None);
    // corners win and edges are hidden on a tiny box
    assert_eq!(
        handle_at(&r(0.0, 0.0, 1.0, 1.0), 0.0, 0.5),
        Some(Handle::Nw)
    );
}

#[test]
fn the_reach_of_a_handle_is_6_px_from_its_centre_in_either_direction_and_no_more() {
    let b = r(10.0, 20.0, 100.0, 60.0);
    // the north west corner is at (10, 20): 4 for half the handle and 2 for the slop
    for (x, y) in [
        (16.0, 20.0),
        (4.0, 20.0),
        (10.0, 26.0),
        (10.0, 14.0),
        (16.0, 26.0),
    ] {
        assert_eq!(handle_at(&b, x, y), Some(Handle::Nw), "({x}, {y})");
    }
    for (x, y) in [
        (16.01, 20.0),
        (3.99, 20.0),
        (10.0, 26.01),
        (10.0, 13.99),
        (16.01, 26.01),
    ] {
        assert_eq!(handle_at(&b, x, y), None, "({x}, {y})");
    }
}

#[test]
fn on_a_tie_the_handle_that_comes_first_in_the_order_nw_n_ne_e_se_s_sw_w_wins() {
    // the smallest box that has edge handles: 24 by 24, so the north handle is 12 px from each corner
    let b = r(0.0, 0.0, 24.0, 24.0);
    assert_eq!(
        handle_at(&b, 6.0, 0.0),
        Some(Handle::Nw),
        "6 from the corner and 6 from the north handle"
    );
    assert_eq!(
        handle_at(&b, 18.0, 0.0),
        Some(Handle::N),
        "6 from the north handle and 6 from the north east corner"
    );
    assert_eq!(handle_at(&b, 24.0, 6.0), Some(Handle::Ne));
    assert_eq!(handle_at(&b, 24.0, 18.0), Some(Handle::E));
    // nearer wins over earlier
    assert_eq!(handle_at(&b, 7.0, 0.0), Some(Handle::N));
}

#[test]
fn the_drop_slot_line_is_the_two_ends_of_the_world_line_on_screen() {
    let cam = Camera {
        x: 100.0,
        y: 20.0,
        zoom: 2.0,
    };
    assert_eq!(
        slot_segment(&cam, &r(10.0, 30.0, 50.0, 0.0)),
        (p(120.0, 80.0), p(220.0, 80.0))
    );
    assert_eq!(
        slot_segment(&cam, &r(40.0, 5.0, 0.0, 20.0)),
        (p(180.0, 30.0), p(180.0, 70.0))
    );
}
