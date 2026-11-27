//! The rules of the camera (E8.1.4, part 1), ported from the TypeScript tests with the same cases and the same expected numbers
//! (`packages/web-canvas/src/camera.test.ts`, `packages/web-shell/src/zoom.test.ts`). The random cases use the same seeded
//! generator, so the same numbers are tried.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app_model::camera::*;
use meldframe_app_model::{
    Camera, CameraOp, Modifiers, Point, Viewport, Wheel, WheelUnit, ZoomCommand,
};
use meldframe_protocol::Node;
use meldframe_render::resolve;
use serde_json::{Value, json};

/// mulberry32, as the TypeScript tests have it, so a failure can be replayed.
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
    fn between(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
    fn camera(&mut self) -> Camera {
        Camera {
            x: self.between(-5000.0, 5000.0),
            y: self.between(-5000.0, 5000.0),
            zoom: self.between(ZOOM_MIN.ln(), ZOOM_MAX.ln()).exp(),
        }
    }
}

fn close(a: f64, b: f64, tol: f64) {
    assert!(
        (a - b).abs() <= tol * 1.0_f64.max(a.abs()).max(b.abs()),
        "{a} is not {b}"
    );
}
fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}
fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}
fn cam(x: f64, y: f64, zoom: f64) -> Camera {
    Camera { x, y, zoom }
}
fn vp(w: f64, h: f64) -> Viewport {
    Viewport { w, h }
}

#[test]
fn world_to_screen_to_world_is_the_identity_for_random_cameras_and_points() {
    let mut g = Rng(1);
    for _ in 0..500 {
        let c = g.camera();
        let (wx, wy) = (g.between(-1e5, 1e5), g.between(-1e5, 1e5));
        let w = to_world(&c, to_screen(&c, p(wx, wy)));
        close(w.x, wx, 1e-6);
        close(w.y, wy, 1e-6);
    }
    let mut g = Rng(2);
    for _ in 0..500 {
        let c = g.camera();
        let (sx, sy) = (g.between(0.0, 2000.0), g.between(0.0, 1200.0));
        let s = to_screen(&c, to_world(&c, p(sx, sy)));
        close(s.x, sx, 1e-6);
        close(s.y, sy, 1e-6);
    }
}

#[test]
fn rectangles_round_trip_and_scale_by_the_zoom() {
    let c = cam(10.0, -20.0, 2.0);
    let s = rect_to_screen(&c, &r(5.0, 5.0, 30.0, 10.0));
    assert_eq!(s, r(20.0, -10.0, 60.0, 20.0));
    assert_eq!(rect_to_world(&c, &s), r(5.0, 5.0, 30.0, 10.0));
    assert_eq!(
        visible_world_rect(&cam(100.0, 50.0, 0.5), vp(400.0, 200.0)),
        r(-200.0, -100.0, 800.0, 400.0)
    );
}

#[test]
fn pan_moves_the_screen_position_and_keeps_the_zoom() {
    assert_eq!(
        pan_by(&cam(1.0, 2.0, 3.0), 10.0, -5.0),
        cam(11.0, -3.0, 3.0)
    );
}

#[test]
fn zoom_at_keeps_the_world_point_under_the_cursor_fixed() {
    let mut g = Rng(3);
    for _ in 0..500 {
        let c = g.camera();
        let at = p(g.between(0.0, 2000.0), g.between(0.0, 1200.0));
        let before = to_world(&c, at);
        let next = zoom_at(&c, g.between(-2.0, 2.0).exp(), at.x, at.y);
        let after = to_world(&next, at);
        close(after.x, before.x, 1e-6);
        close(after.y, before.y, 1e-6);
    }
}

#[test]
fn zoom_is_limited_to_2_percent_and_6400_percent() {
    assert_eq!(clamp_zoom(0.0001), ZOOM_MIN);
    assert_eq!(clamp_zoom(1000.0), ZOOM_MAX);
    assert_eq!((ZOOM_MIN, ZOOM_MAX), (0.02, 64.0));
    let c = zoom_to(&cam(0.0, 0.0, 1.0), 1e9, 100.0, 100.0);
    assert_eq!(c.zoom, ZOOM_MAX);
    // the anchor stays put even when the limit cut the factor
    close(to_world(&c, p(100.0, 100.0)).x, 100.0, 1e-6);
}

#[test]
fn zooming_in_then_out_by_the_same_factor_returns_to_the_start() {
    let mut g = Rng(4);
    for _ in 0..200 {
        let c = Camera {
            zoom: g.between(0.2, 5.0),
            ..g.camera()
        };
        let (sx, sy) = (g.between(0.0, 1000.0), g.between(0.0, 800.0));
        let back = zoom_at(&zoom_at(&c, 1.5, sx, sy), 1.0 / 1.5, sx, sy);
        close(back.zoom, c.zoom, 1e-6);
        close(back.x, c.x, 1e-6);
        close(back.y, c.y, 1e-6);
    }
}

#[test]
fn fit_centres_the_content_and_leaves_the_padding() {
    let v = vp(1000.0, 600.0);
    let o = FitOptions {
        padding: 50.0,
        ..FitOptions::default()
    };
    let c = fit_rect(Some(r(100.0, 100.0, 500.0, 200.0)), v, o);
    close(c.zoom, 1.8, 1e-9); // width: 900 / 500
    let s = rect_to_screen(&c, &r(100.0, 100.0, 500.0, 200.0));
    close(s.x + s.w / 2.0, 500.0, 1e-6);
    close(s.y + s.h / 2.0, 300.0, 1e-6);
    assert!(s.x >= 50.0 - 1e-9 && s.x + s.w <= 950.0 + 1e-9);
}

#[test]
fn fit_always_shows_the_whole_rectangle() {
    let v = vp(1000.0, 600.0);
    let o = FitOptions {
        padding: 20.0,
        ..FitOptions::default()
    };
    let mut g = Rng(5);
    for _ in 0..300 {
        let rect = r(
            g.between(-1e4, 1e4),
            g.between(-1e4, 1e4),
            g.between(1.0, 1e4),
            g.between(1.0, 1e4),
        );
        let c = fit_rect(Some(rect), v, o);
        let s = rect_to_screen(&c, &rect);
        if c.zoom > ZOOM_MIN && c.zoom < ZOOM_MAX {
            assert!(s.x >= 20.0 - 1e-6 && s.y >= 20.0 - 1e-6);
            assert!(s.x + s.w <= v.w - 20.0 + 1e-6 && s.y + s.h <= v.h - 20.0 + 1e-6);
        }
    }
}

#[test]
fn fit_honours_a_zoom_cap_and_the_limits_and_centres_the_origin_for_no_content() {
    let v = vp(1000.0, 600.0);
    let cap = FitOptions {
        max_zoom: 1.0,
        ..FitOptions::default()
    };
    let d = FitOptions::default();
    assert_eq!((d.padding, d.max_zoom), (FIT_PADDING, ZOOM_MAX));
    assert_eq!(fit_rect(Some(r(0.0, 0.0, 10.0, 10.0)), v, cap).zoom, 1.0);
    assert_eq!(fit_rect(Some(r(0.0, 0.0, 1e9, 1e9)), v, d).zoom, ZOOM_MIN);
    assert_eq!(fit_rect(None, v, d), cam(500.0, 300.0, 1.0));
    // a rectangle with no area is centred at its point at 100%
    assert_eq!(
        fit_rect(Some(r(10.0, 20.0, 0.0, 5.0)), v, d),
        cam(490.0, 280.0, 1.0)
    );
}

fn wheel(dx: f64, dy: f64, unit: WheelUnit, mods: Modifiers) -> Wheel {
    Wheel { dx, dy, unit, mods }
}
const PLAIN: Modifiers = Modifiers {
    ctrl: false,
    meta: false,
    alt: false,
    shift: false,
};
const CTRL: Modifiers = Modifiers {
    ctrl: true,
    ..PLAIN
};

#[test]
fn a_plain_wheel_pans_against_the_delta_and_line_and_page_deltas_are_pixels() {
    let v = vp(800.0, 600.0);
    let pan = |w| wheel_action(&w, v);
    assert_eq!(
        pan(wheel(5.0, 30.0, WheelUnit::Pixel, PLAIN)),
        WheelAction::Pan {
            dx: -5.0,
            dy: -30.0
        }
    );
    assert_eq!(
        pan(wheel(0.0, 3.0, WheelUnit::Line, PLAIN)),
        WheelAction::Pan { dx: 0.0, dy: -48.0 }
    );
    assert_eq!(
        pan(wheel(0.0, 1.0, WheelUnit::Page, PLAIN)),
        WheelAction::Pan {
            dx: 0.0,
            dy: -600.0
        }
    );
}

#[test]
fn ctrl_or_meta_zooms_up_in_down_out_and_one_notch_is_limited() {
    let v = vp(800.0, 600.0);
    let meta = Modifiers {
        meta: true,
        ..PLAIN
    };
    let factor = |w| match wheel_action(&w, v) {
        WheelAction::Zoom { factor } => factor,
        other => panic!("{other:?}"),
    };
    assert!(factor(wheel(0.0, -10.0, WheelUnit::Pixel, CTRL)) > 1.0);
    assert!(factor(wheel(0.0, 10.0, WheelUnit::Pixel, meta)) < 1.0);
    assert!(factor(wheel(0.0, -5000.0, WheelUnit::Pixel, CTRL)) < 2.0);
}

#[test]
fn a_wheel_zoom_keeps_the_cursor_point_fixed() {
    let c = cam(30.0, 40.0, 1.3);
    let at = p(200.0, 150.0);
    let before = to_world(&c, at);
    let next = apply_wheel(
        &c,
        &wheel(0.0, -20.0, WheelUnit::Pixel, CTRL),
        at,
        vp(800.0, 600.0),
    );
    let after = to_world(&next, at);
    close(after.x, before.x, 1e-6);
    close(after.y, before.y, 1e-6);
    assert!(next.zoom > c.zoom);
}

#[test]
fn a_pinch_keeps_the_world_points_under_both_fingers_under_them() {
    let mut g = Rng(6);
    for _ in 0..300 {
        let c = Camera {
            zoom: g.between(0.1, 8.0),
            ..g.camera()
        };
        let a0 = p(g.between(0.0, 800.0), g.between(0.0, 600.0));
        let b0 = p(a0.x + g.between(50.0, 300.0), a0.y + g.between(50.0, 300.0));
        let k = g.between(0.5, 2.0);
        // a pure scale around the midpoint plus a shift: both fingers' world points must follow
        let m = p((a0.x + b0.x) / 2.0, (a0.y + b0.y) / 2.0);
        let shift = p(g.between(-100.0, 100.0), g.between(-100.0, 100.0));
        let moved = |f: Point| {
            p(
                m.x + (f.x - m.x) * k + shift.x,
                m.y + (f.y - m.y) * k + shift.y,
            )
        };
        let (a1, b1) = (moved(a0), moved(b0));
        let next = pinch(&c, [a0, b0], [a1, b1]);
        if next.zoom <= ZOOM_MIN || next.zoom >= ZOOM_MAX {
            continue;
        }
        for (from, to) in [(a0, a1), (b0, b1)] {
            let s = to_screen(&next, to_world(&c, from));
            close(s.x, to.x, 1e-6);
            close(s.y, to.y, 1e-6);
        }
    }
}

#[test]
fn a_pinch_with_coincident_fingers_changes_nothing_and_the_zoom_stays_within_the_limits() {
    let c = cam(1.0, 2.0, 1.0);
    let at = p(5.0, 5.0);
    assert_eq!(pinch(&c, [at, at], [at, p(9.0, 9.0)]), c);
    let far = pinch(
        &cam(0.0, 0.0, 60.0),
        [p(0.0, 0.0), p(10.0, 0.0)],
        [p(0.0, 0.0), p(1000.0, 0.0)],
    );
    assert_eq!(far.zoom, ZOOM_MAX);
}

#[test]
fn the_readout_shows_a_whole_percentage() {
    assert_eq!(zoom_percent(1.0), 100);
    for (zoom, text) in [(0.05, "5%"), (0.02, "2%"), (64.0, "6400%"), (1.234, "123%")] {
        assert_eq!(format_zoom(zoom), text);
    }
}

#[test]
fn the_zoom_commands_take_one_step_of_the_keys_and_100_percent_sets_1_around_the_centre_of_the_view()
 {
    use meldframe_app_model::ZoomCommand::{In, Out, Reset};
    let c = cam(120.0, -40.0, 0.5);
    let v = vp(600.0, 400.0);
    let centre = to_world(&c, p(v.w / 2.0, v.h / 2.0));
    for (cmd, zoom) in [(In, 0.5 * ZOOM_STEP), (Out, 0.5 / ZOOM_STEP), (Reset, 1.0)] {
        let next = zoom_command(&c, v, cmd);
        close(next.zoom, zoom, 1e-12);
        // the centre of the view stays where it is on the screen
        let at = to_screen(&next, centre);
        close(at.x, v.w / 2.0, 1e-9);
        close(at.y, v.h / 2.0, 1e-9);
    }
}

#[test]
fn the_zoom_commands_never_leave_the_zoom_limits() {
    use meldframe_app_model::ZoomCommand::{In, Out};
    let v = vp(600.0, 400.0);
    assert_eq!(zoom_command(&cam(1.0, 2.0, ZOOM_MAX), v, In).zoom, ZOOM_MAX);
    assert_eq!(
        zoom_command(&cam(1.0, 2.0, ZOOM_MIN), v, Out).zoom,
        ZOOM_MIN
    );
}

#[test]
fn a_zoom_that_stays_where_it_is_leaves_the_camera_exactly_as_it_was() {
    // 100% on a camera that is at 100% already: no round-off in the offset (0.1 stayed 0.10000000000002274)
    let c = cam(0.1, 0.7, 1.0);
    assert_eq!(zoom_to(&c, 1.0, 500.0, 300.0), c);
    assert_eq!(zoom_command(&c, vp(1000.0, 600.0), ZoomCommand::Reset), c);
    // at the limits a step that the limit cuts away changes nothing, at either end
    let top = cam(0.1, 0.7, ZOOM_MAX);
    assert_eq!(zoom_at(&top, 2.0, 12.0, 34.0), top);
    let bottom = cam(0.1, 0.7, ZOOM_MIN);
    assert_eq!(zoom_at(&bottom, 0.5, 12.0, 34.0), bottom);
    assert_eq!(zoom_to(&bottom, 0.0, 12.0, 34.0), bottom);
}

#[test]
fn a_fit_with_no_viewport_to_fit_in_changes_nothing() {
    let c = cam(3.0, 4.0, 2.0);
    let everything = || Bounds {
        content: Some(r(0.0, 0.0, 100.0, 100.0)),
        selection: Some(r(0.0, 0.0, 10.0, 10.0)),
    };
    for command in [ZoomCommand::Fit, ZoomCommand::FitSelection] {
        let op = CameraOp::Zoom { command };
        for v in [vp(0.0, 0.0), vp(0.0, 600.0), vp(1000.0, 0.0)] {
            assert_eq!(
                apply(&op, c, v, everything),
                Some((c, v)),
                "{command:?} {v:?}"
            );
        }
    }
}

// What a fit looks at (`bounds`): the world rectangles, which a box holds as offsets inside its parent.

fn nodes_of(list: Value) -> Vec<Node> {
    serde_json::from_value(list).expect("a valid value")
}

fn some(x: f64, y: f64, w: f64, h: f64) -> Option<Rect> {
    Some(Rect { x, y, w, h })
}

/// A frame at (500, 300), a frame in it at (20, 20) and a rectangle in that at (5, 5), and a rectangle `c` in the first at (10, 10).
fn nested() -> Value {
    json!([
        {"id": "a", "type": "frame", "pos": "a0", "x": 500, "y": 300, "w": 200, "h": 100},
        {"id": "c", "type": "rect", "parentId": "a", "pos": "a0", "x": 10, "y": 10, "w": 50, "h": 50},
        {"id": "m", "type": "frame", "parentId": "a", "pos": "a1", "x": 20, "y": 20, "w": 100, "h": 60},
        {"id": "g", "type": "rect", "parentId": "m", "pos": "a0", "x": 5, "y": 5, "w": 10, "h": 10},
    ])
}

fn bounds_of(list: Value, selection: &[&str]) -> Bounds {
    bounds(
        &resolve(&nodes_of(list)),
        &selection.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )
}

#[test]
fn a_selected_node_is_where_the_canvas_draws_it_not_where_its_parent_holds_it() {
    let b = bounds_of(nested(), &["c"]);
    assert_eq!(b.selection, some(510.0, 310.0, 50.0, 50.0));
    // a grandchild: the offsets of both ancestors are in it
    assert_eq!(
        bounds_of(nested(), &["g"]).selection,
        some(525.0, 325.0, 10.0, 10.0)
    );
    // several nodes, in any order, give their union; an id that is not a node adds nothing
    let b = bounds_of(nested(), &["g", "ghost", "c"]);
    assert_eq!(b.selection, some(510.0, 310.0, 50.0, 50.0));
    assert_eq!(bounds_of(nested(), &["ghost"]).selection, None);
    assert_eq!(bounds_of(nested(), &[]).selection, None);
}

#[test]
fn the_content_is_what_is_on_the_canvas_and_a_child_that_overflows_its_frame_counts_unless_the_frame_clips()
 {
    let overflow = |clip: bool| {
        json!([
            {"id": "a", "type": "frame", "pos": "a0", "x": 100, "y": 100, "w": 200, "h": 100, "clip": clip},
            {"id": "c", "type": "rect", "parentId": "a", "pos": "a0", "x": 150, "y": 50, "w": 100, "h": 100},
        ])
    };
    assert_eq!(
        bounds_of(overflow(false), &[]).content,
        some(100.0, 100.0, 250.0, 150.0)
    );
    // the part the frame cuts away is not on the canvas
    assert_eq!(
        bounds_of(overflow(true), &[]).content,
        some(100.0, 100.0, 200.0, 100.0)
    );
    // two roots
    assert_eq!(
        bounds_of(nested(), &[]).content,
        some(500.0, 300.0, 200.0, 100.0),
        "everything of the nested file lies inside its frame"
    );
    assert_eq!(bounds_of(json!([]), &["c"]), Bounds::default());
}
