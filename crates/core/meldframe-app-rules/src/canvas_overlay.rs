//! The overlay of the canvas that does not scale with the zoom (E8.1.4, part 1): the selection box and its resize handles are
//! placed through the camera, and their size is in screen pixels whatever the zoom, so a reach in pixels is a reach of `px / zoom`
//! in the world. Ported from `packages/web-canvas/src/overlay.ts`; the drawing is the front end's.

use meldframe_app_model::{Camera, Handle, Point};
use meldframe_camera::{Rect, rect_to_screen};

/// Side of a square resize handle, in screen pixels.
pub const HANDLE_SIZE: f64 = 8.0;
/// Extra reach around a handle for the pointer, in screen pixels.
pub const HANDLE_SLOP: f64 = 2.0;
/// An edge handle is only offered when the box is at least this long and this wide, so handles never crowd the body.
pub const HANDLE_MIN_SIDE: f64 = 24.0;

/// The handles in the order they are drawn and tried.
pub const HANDLES: [Handle; 8] = [
    Handle::Nw,
    Handle::N,
    Handle::Ne,
    Handle::E,
    Handle::Se,
    Handle::S,
    Handle::Sw,
    Handle::W,
];

/// A length in screen pixels as world units at the camera's zoom: what a size that does not scale comes to in the world.
pub fn world_len(px: f64, c: &Camera) -> f64 {
    px / c.zoom
}

/// The screen box of the selection: the union of the WORLD rectangles through the camera; none for none. A painted box holds its
/// offset inside its parent, so for a node in a frame the world rectangle is not its `x` and `y` (`meldframe_render::extent_of`
/// and `HitIndex::rect_of` give it, with the offsets of the ancestors summed).
pub fn selection_box(c: &Camera, world: &[Rect]) -> Option<Rect> {
    let first = *world.first()?;
    let u = world.iter().fold(first, |a, r| {
        let (x, y) = (a.x.min(r.x), a.y.min(r.y));
        Rect {
            x,
            y,
            w: (a.x + a.w).max(r.x + r.w) - x,
            h: (a.y + a.h).max(r.y + r.h) - y,
        }
    });
    Some(rect_to_screen(c, &u))
}

/// The centre of a handle on a screen box.
pub fn handle_position(b: &Rect, h: Handle) -> Point {
    let (west, east, north, south) = h.edges();
    let pick = |lo: bool, hi: bool, start: f64, len: f64| match (lo, hi) {
        (true, _) => start,
        (_, true) => start + len,
        _ => start + len / 2.0,
    };
    Point {
        x: pick(west, east, b.x, b.w),
        y: pick(north, south, b.y, b.h),
    }
}

/// The handles to draw and to hit: the corners always, an edge only when the box is long enough.
pub fn visible_handles(b: &Rect) -> Vec<Handle> {
    let roomy = b.w >= HANDLE_MIN_SIDE && b.h >= HANDLE_MIN_SIDE;
    HANDLES
        .into_iter()
        .filter(|h| roomy || matches!(h, Handle::Nw | Handle::Ne | Handle::Se | Handle::Sw))
        .collect()
}

/// The handle under the screen point, or none. The nearest centre wins; on a tie the first in the order of [`HANDLES`] (nw, n,
/// ne, e, se, s, sw, w) does.
pub fn handle_at(b: &Rect, px: f64, py: f64) -> Option<Handle> {
    let reach = HANDLE_SIZE / 2.0 + HANDLE_SLOP;
    let mut best = None;
    let mut best_d = f64::INFINITY;
    for h in visible_handles(b) {
        let at = handle_position(b, h);
        let d = (px - at.x).abs().max((py - at.y).abs());
        if d <= reach && d < best_d {
            best = Some(h);
            best_d = d;
        }
    }
    best
}

/// The two ends on screen of the line of a drop slot (zero thick on one axis, a rectangle in WORLD units, as `DropSlot` gives it).
pub fn slot_segment(c: &Camera, line: &Rect) -> (Point, Point) {
    let r = rect_to_screen(c, line);
    (
        Point { x: r.x, y: r.y },
        Point {
            x: r.x + r.w,
            y: r.y + r.h,
        },
    )
}
