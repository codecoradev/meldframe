//! The camera of the canvas (E8.1.4, part 1): the limits of the zoom, the mapping between the world and the screen, pan, zoom
//! around a point, the fit of a rectangle, the meaning of a wheel and a pinch, and the zoom commands, as pure functions of a
//! [`Camera`] and a [`Viewport`]. The rules were the TypeScript ones of `packages/web-canvas/src/camera.ts` and
//! `packages/web-shell/src/zoom.ts`, with their tests.

use crate::input::Point;
use crate::types::{Camera, CameraOp, Viewport, Wheel, WheelUnit, ZoomCommand};
pub use meldframe_layout::Rect;
use meldframe_render::{HitIndex, PaintedBox, content_extent, extent_of};

pub const ZOOM_MIN: f64 = 0.02;
pub const ZOOM_MAX: f64 = 64.0;
/// Fit leaves this much room (CSS pixels) around the content.
pub const FIT_PADDING: f64 = 48.0;
/// A step of the zoom in and zoom out commands and keys.
pub const ZOOM_STEP: f64 = 1.25;
/// A line of a wheel in line units is this many pixels.
const LINE_PX: f64 = 16.0;
/// One wheel tick of a mouse zooms by about this much at most: the delta is limited so a notch is a step.
const WHEEL_ZOOM_CLAMP: f64 = 40.0;
const WHEEL_ZOOM_RATE: f64 = 0.01;

pub fn clamp_zoom(zoom: f64) -> f64 {
    zoom.clamp(ZOOM_MIN, ZOOM_MAX)
}

pub fn to_screen(c: &Camera, p: Point) -> Point {
    Point {
        x: p.x * c.zoom + c.x,
        y: p.y * c.zoom + c.y,
    }
}

pub fn to_world(c: &Camera, p: Point) -> Point {
    Point {
        x: (p.x - c.x) / c.zoom,
        y: (p.y - c.y) / c.zoom,
    }
}

pub fn rect_to_screen(c: &Camera, r: &Rect) -> Rect {
    Rect {
        x: r.x * c.zoom + c.x,
        y: r.y * c.zoom + c.y,
        w: r.w * c.zoom,
        h: r.h * c.zoom,
    }
}

pub fn rect_to_world(c: &Camera, r: &Rect) -> Rect {
    Rect {
        x: (r.x - c.x) / c.zoom,
        y: (r.y - c.y) / c.zoom,
        w: r.w / c.zoom,
        h: r.h / c.zoom,
    }
}

/// The world rectangle the viewport shows.
pub fn visible_world_rect(c: &Camera, v: Viewport) -> Rect {
    rect_to_world(
        c,
        &Rect {
            x: 0.0,
            y: 0.0,
            w: v.w,
            h: v.h,
        },
    )
}

pub fn pan_by(c: &Camera, dx: f64, dy: f64) -> Camera {
    Camera {
        x: c.x + dx,
        y: c.y + dy,
        ..*c
    }
}

/// Sets the zoom (cut to the limits) and keeps the world point under the screen point `(sx, sy)` where it is. A zoom that is the
/// one the camera has leaves it exactly as it is (no round-off in the offset).
pub fn zoom_to(c: &Camera, zoom: f64, sx: f64, sy: f64) -> Camera {
    let zoom = clamp_zoom(zoom);
    if zoom == c.zoom {
        return *c;
    }
    let w = to_world(c, Point { x: sx, y: sy });
    Camera {
        x: sx - w.x * zoom,
        y: sy - w.y * zoom,
        zoom,
    }
}

/// Multiplies the zoom by `factor` around the screen point `(sx, sy)`.
pub fn zoom_at(c: &Camera, factor: f64, sx: f64, sy: f64) -> Camera {
    zoom_to(c, c.zoom * factor, sx, sy)
}

/// How a fit leaves room: the padding around the rectangle, and the largest zoom a fit may give.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitOptions {
    pub padding: f64,
    pub max_zoom: f64,
}

impl Default for FitOptions {
    fn default() -> Self {
        FitOptions {
            padding: FIT_PADDING,
            max_zoom: ZOOM_MAX,
        }
    }
}

/// The camera that shows `r` centred in the viewport; for no rectangle the origin, centred at 100%, and for one with no area
/// its point, centred at 100%.
pub fn fit_rect(r: Option<Rect>, v: Viewport, o: FitOptions) -> Camera {
    let Some(r) = r.filter(|r| r.w > 0.0 && r.h > 0.0) else {
        let (cx, cy) = r.map_or((0.0, 0.0), |r| (r.x, r.y));
        return Camera {
            x: v.w / 2.0 - cx,
            y: v.h / 2.0 - cy,
            zoom: 1.0,
        };
    };
    let avail_w = (v.w - 2.0 * o.padding).max(1.0);
    let avail_h = (v.h - 2.0 * o.padding).max(1.0);
    let zoom = clamp_zoom((avail_w / r.w).min(avail_h / r.h)).min(o.max_zoom);
    Camera {
        x: v.w / 2.0 - (r.x + r.w / 2.0) * zoom,
        y: v.h / 2.0 - (r.y + r.h / 2.0) * zoom,
        zoom,
    }
}

/// The zoom as the percentage the readout shows (100 at zoom 1), rounded as JavaScript's `Math.round` does.
pub fn zoom_percent(zoom: f64) -> i64 {
    (zoom * 100.0 + 0.5).floor() as i64
}

pub fn format_zoom(zoom: f64) -> String {
    format!("{}%", zoom_percent(zoom))
}

/// What a wheel event means.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WheelAction {
    Pan { dx: f64, dy: f64 },
    Zoom { factor: f64 },
}

/// The wheel pans (against the delta); with Ctrl or Command held it zooms, up zooming in.
pub fn wheel_action(w: &Wheel, v: Viewport) -> WheelAction {
    let unit = match w.unit {
        WheelUnit::Pixel => 1.0,
        WheelUnit::Line => LINE_PX,
        WheelUnit::Page => v.h,
    };
    if w.mods.command() {
        let d = (w.dy * unit).clamp(-WHEEL_ZOOM_CLAMP, WHEEL_ZOOM_CLAMP);
        return WheelAction::Zoom {
            factor: (-d * WHEEL_ZOOM_RATE).exp(),
        };
    }
    WheelAction::Pan {
        dx: -(w.dx * unit),
        dy: -(w.dy * unit),
    }
}

/// Applies a wheel event at the screen point `at`.
pub fn apply_wheel(c: &Camera, w: &Wheel, at: Point, v: Viewport) -> Camera {
    match wheel_action(w, v) {
        WheelAction::Pan { dx, dy } => pan_by(c, dx, dy),
        WheelAction::Zoom { factor } => zoom_at(c, factor, at.x, at.y),
    }
}

fn mid(p: [Point; 2]) -> Point {
    Point {
        x: (p[0].x + p[1].x) / 2.0,
        y: (p[0].y + p[1].y) / 2.0,
    }
}

fn dist(p: [Point; 2]) -> f64 {
    (p[0].x - p[1].x).hypot(p[0].y - p[1].y)
}

/// A two-pointer gesture from the pair `prev` to the pair `next`: the zoom follows the change in distance, and the world point
/// that was under the old midpoint ends under the new one. Coincident fingers change nothing.
pub fn pinch(c: &Camera, prev: [Point; 2], next: [Point; 2]) -> Camera {
    let d0 = dist(prev);
    if d0 < 1e-6 {
        return *c;
    }
    let (m0, m1) = (mid(prev), mid(next));
    let zoom = clamp_zoom(c.zoom * (dist(next) / d0));
    let w = to_world(c, m0);
    Camera {
        x: m1.x - w.x * zoom,
        y: m1.y - w.y * zoom,
        zoom,
    }
}

/// In, out or 100% around the centre of the view, which stays where it is on the screen. The two fits need the content: they
/// are [`apply`]'s.
pub fn zoom_command(c: &Camera, v: Viewport, cmd: ZoomCommand) -> Camera {
    let (sx, sy) = (v.w / 2.0, v.h / 2.0);
    match cmd {
        ZoomCommand::In => zoom_at(c, ZOOM_STEP, sx, sy),
        ZoomCommand::Out => zoom_at(c, 1.0 / ZOOM_STEP, sx, sy),
        ZoomCommand::Reset => zoom_to(c, 1.0, sx, sy),
        ZoomCommand::Fit | ZoomCommand::FitSelection => *c,
    }
}

/// The rectangles a fit looks at, in world space: everything that is on the canvas (`meldframe_render::content_extent`: every
/// box, with what a clipping ancestor cuts away left out) and the selected nodes (`extent_of`); none for none.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Bounds {
    pub content: Option<Rect>,
    pub selection: Option<Rect>,
}

/// The bounds of the content and of the `selection` in the painted `boxes`. A box holds its offset inside its parent: the world
/// rectangles are those of the hit index, which sums the offsets of the ancestors (the one place that does).
pub fn bounds(boxes: &[PaintedBox], selection: &[String]) -> Bounds {
    let index = HitIndex::from_boxes(boxes);
    Bounds {
        content: content_extent(&index),
        selection: extent_of(&index, selection),
    }
}

fn finite(c: &Camera) -> bool {
    c.x.is_finite() && c.y.is_finite() && c.zoom.is_finite()
}

fn all_finite(v: &[f64]) -> bool {
    v.iter().all(|n| n.is_finite())
}

/// What the op makes of the camera and the viewport, or none for an op that is refused: a number that is not finite, a factor
/// that is not above 0, a negative size, or a result that is not finite. `bounds` is asked only for a fit.
pub fn apply(
    op: &CameraOp,
    c: Camera,
    v: Viewport,
    bounds: impl FnOnce() -> Bounds,
) -> Option<(Camera, Viewport)> {
    let next = match *op {
        CameraOp::Pan { dx, dy } if all_finite(&[dx, dy]) => pan_by(&c, dx, dy),
        CameraOp::ZoomAt { factor, at } if all_finite(&[factor, at.x, at.y]) && factor > 0.0 => {
            zoom_at(&c, factor, at.x, at.y)
        }
        // Nothing to fit in until the front end has said how large the canvas is.
        CameraOp::Zoom {
            command: ZoomCommand::Fit | ZoomCommand::FitSelection,
        } if v.w <= 0.0 || v.h <= 0.0 => return Some((c, v)),
        CameraOp::Zoom { command } => match command {
            ZoomCommand::Fit => fit_rect(bounds().content, v, FitOptions::default()),
            ZoomCommand::FitSelection => {
                let b = bounds();
                fit_rect(b.selection.or(b.content), v, FitOptions::default())
            }
            _ => zoom_command(&c, v, command),
        },
        CameraOp::Wheel { wheel, at } if all_finite(&[wheel.dx, wheel.dy, at.x, at.y]) => {
            apply_wheel(&c, &wheel, at, v)
        }
        CameraOp::Pinch { from, to }
            if all_finite(&[from[0].x, from[0].y, from[1].x, from[1].y])
                && all_finite(&[to[0].x, to[0].y, to[1].x, to[1].y]) =>
        {
            pinch(&c, from, to)
        }
        CameraOp::SetViewport { w, h } if all_finite(&[w, h]) && w >= 0.0 && h >= 0.0 => {
            return Some((c, Viewport { w, h }));
        }
        _ => return None,
    };
    finite(&next).then_some((next, v))
}
