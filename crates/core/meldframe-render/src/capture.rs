//! The rectangle a capture or a PNG export covers (finding T13 of the thin UI audit, decision 43): a box with the room its
//! paint takes beyond it (`PaintedBox::paint_bounds`, T16), in world space. The canvas's `captureRectOf` and `captureBounds`
//! worked this out in TypeScript from the same scene; this is the one rule, read by the app layer.

use crate::hit::{HitIndex, overlap};
use crate::{PaintBounds, PaintedBox};
use meldframe_layout::Rect;
use std::collections::HashMap;

/// The box grown by the reach of its paint on each side.
fn painted(r: Rect, p: &PaintBounds) -> Rect {
    Rect {
        x: r.x - p.left,
        y: r.y - p.top,
        w: r.w + p.left + p.right,
        h: r.h + p.top + p.bottom,
    }
}

/// The world rectangle to capture for the box `id`, with its outside border and its shadows in it (what a clipping ancestor
/// cuts is in it: a capture of a node shows what the node paints). `None` for an id that is not a box; the first box wins
/// when an id is repeated.
pub fn capture_rect(boxes: &[PaintedBox], id: &str) -> Option<Rect> {
    capture_rect_in(&HitIndex::from_boxes(boxes), boxes, id)
}

/// [`capture_rect`] with the index of `boxes` the caller already holds (`HitIndex::from_boxes`), so a caller that keeps one
/// per scene does not build it for each question. The index must be that of `boxes`.
pub fn capture_rect_in(index: &HitIndex, boxes: &[PaintedBox], id: &str) -> Option<Rect> {
    let bounds = boxes.iter().find(|b| b.id == id)?.paint_bounds;
    let (_, rect, _) = index.placed().find(|(i, ..)| *i == id)?;
    Some(painted(rect, &bounds))
}

/// The rectangle around every box as a capture takes it (outside borders and shadows included), or `None` for an empty
/// scene. What a clipping ancestor cuts is not in it, as it is not on the canvas; a box inside a frame that does not clip
/// counts, so a shadow that reaches out of its page is in the picture. A box that is cut away whole adds nothing.
pub fn capture_bounds(boxes: &[PaintedBox]) -> Option<Rect> {
    capture_bounds_in(&HitIndex::from_boxes(boxes), boxes)
}

/// [`capture_bounds`] with the index of `boxes` the caller already holds (see [`capture_rect_in`]).
pub fn capture_bounds_in(index: &HitIndex, boxes: &[PaintedBox]) -> Option<Rect> {
    let mut reach: HashMap<&str, &PaintBounds> = HashMap::with_capacity(boxes.len());
    for b in boxes {
        reach.entry(b.id.as_str()).or_insert(&b.paint_bounds);
    }
    let mut out: Option<Rect> = None;
    for (id, rect, clip) in index.placed() {
        let own = painted(rect, reach[id]);
        let r = match clip {
            Some(c) => overlap(own, c),
            None => own,
        };
        if clip.is_some() && !(r.w > 0.0 && r.h > 0.0) {
            continue;
        }
        out = Some(out.map_or(r, |o| union(o, r)));
    }
    out
}

pub(crate) fn union(a: Rect, b: Rect) -> Rect {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    Rect {
        x,
        y,
        w: (a.x + a.w).max(b.x + b.w) - x,
        h: (a.y + a.h).max(b.y + b.h) - y,
    }
}
