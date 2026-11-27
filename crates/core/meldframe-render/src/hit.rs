//! What is under a point: the one definition of a hit (T9 of the thin UI audit, decision 43).
//!
//! A click, the hover, the tool that creates and a drop all ask the same question of the same boxes, and
//! [`HitIndex`] is where it is answered. A box is under a point when
//!
//! - the box is visible, and so is every ancestor (a hidden box hides its subtree);
//! - the point is inside its rectangle in world space, the left and top edges in and the right and bottom edges out, so
//!   two boxes that share an edge never both own a point on it, and a box with no width or no height owns none;
//! - the point is inside the clip of every ancestor that clips: a box that clips (`clip`) cuts its children, and
//!   what they hold, to its own rectangle, and the cuts of nested boxes add up (the part that is left is the overlap of
//!   them all). A box is not cut by its own clip.
//!
//! The boxes are in paint order, back to front, a parent before its children (what `resolve_with` gives), so the
//! front-most box is the last one that qualifies. [`HitIndex::under`] lists them front to back; a click wants the first,
//! a drop wants the first frame that is not the node dragged, and both read the same list. The index is a plain value
//! built once for a scene and asked as often as needed: it reads no state of the app.
//!
//! Visibility: [`PaintedBox`] has no visibility field yet (ADR-0025, which would add one, is proposed), so every box made
//! by [`HitIndex::from_boxes`] is visible and the rule has nothing to hide. It is here and tested on [`HitBox`], so the
//! field is handed over in one place when it exists.

use crate::PaintedBox;
use meldframe_layout::Rect;
use std::borrow::Cow;
use std::collections::HashMap;

/// One box as the hit rule reads it: where it is in world space, whether it clips its children and whether it is shown.
/// `parent` names a box that is listed before this one; a parent that is not (it is missing, or listed later) leaves the
/// box as a root.
#[derive(Debug, Clone, Copy)]
pub struct HitBox<'a> {
    pub id: &'a str,
    pub parent: Option<&'a str>,
    /// In world space.
    pub rect: Rect,
    pub clip: bool,
    /// Its own visibility; a hidden ancestor hides it too.
    pub visible: bool,
}

/// What an index keeps of a box.
#[derive(Debug, Clone)]
struct Slot<'a> {
    id: Cow<'a, str>,
    rect: Rect,
    /// The overlap of the clips of the ancestors that clip: the part of the box that can be hit; none when nothing clips it.
    clip: Option<Rect>,
    /// Visible itself and through every ancestor.
    shown: bool,
    /// What its children inherit as their clip.
    inherited: Option<Rect>,
}

/// The boxes of a scene in paint order, with what each one inherits worked out, ready to be asked what is under a point.
#[derive(Debug, Clone, Default)]
pub struct HitIndex<'a> {
    slots: Vec<Slot<'a>>,
}

impl<'a> HitIndex<'a> {
    /// The index of boxes in paint order. A repeated id keeps its first box (a later one with that id is left out).
    pub fn new(boxes: impl IntoIterator<Item = HitBox<'a>>) -> Self {
        let mut slots: Vec<Slot<'a>> = Vec::new();
        let mut at: HashMap<&'a str, usize> = HashMap::new();
        for b in boxes {
            if at.contains_key(b.id) {
                continue;
            }
            let parent = b.parent.and_then(|p| at.get(p)).map(|&i| &slots[i]);
            let clip = parent.and_then(|p| p.inherited);
            let shown = b.visible && parent.is_none_or(|p| p.shown);
            let inherited = if b.clip {
                Some(clip.map_or(b.rect, |c| overlap(c, b.rect)))
            } else {
                clip
            };
            at.insert(b.id, slots.len());
            slots.push(Slot {
                id: Cow::Borrowed(b.id),
                rect: b.rect,
                clip,
                shown,
                inherited,
            });
        }
        HitIndex { slots }
    }

    /// The index of the painted boxes of a scene. A box holds its offset inside its parent, so the world rectangle is the
    /// sum of the offsets of its ancestors (the parent comes first in the list, as `resolve_with` gives it). Every box is
    /// visible (see the module note).
    pub fn from_boxes(boxes: &[PaintedBox]) -> HitIndex<'static> {
        let mut origin: HashMap<&str, (f64, f64)> = HashMap::with_capacity(boxes.len());
        let mut placed = Vec::with_capacity(boxes.len());
        for b in boxes {
            let (px, py) = b
                .parent
                .as_deref()
                .and_then(|p| origin.get(p))
                .copied()
                .unwrap_or((0.0, 0.0));
            let rect = Rect {
                x: px + b.x,
                y: py + b.y,
                w: b.w,
                h: b.h,
            };
            origin.entry(b.id.as_str()).or_insert((rect.x, rect.y));
            placed.push(HitBox {
                id: &b.id,
                parent: b.parent.as_deref(),
                rect,
                clip: b.clip,
                visible: true,
            });
        }
        HitIndex::new(placed).into_owned()
    }

    /// The same index with no borrow of the boxes it was made from.
    pub fn into_owned(self) -> HitIndex<'static> {
        HitIndex {
            slots: self
                .slots
                .into_iter()
                .map(|s| Slot {
                    id: Cow::Owned(s.id.into_owned()),
                    rect: s.rect,
                    clip: s.clip,
                    shown: s.shown,
                    inherited: s.inherited,
                })
                .collect(),
        }
    }

    /// How many boxes the index holds.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// The ids of the boxes the point is over, front-most first. A point that is not a number is over nothing.
    pub fn under(&self, x: f64, y: f64) -> impl Iterator<Item = &str> {
        self.slots
            .iter()
            .rev()
            .filter(move |s| {
                s.shown
                    && contains_point(s.rect, x, y)
                    && s.clip.is_none_or(|c| contains_point(c, x, y))
            })
            .map(|s| s.id.as_ref())
    }

    /// The front-most box under the point.
    pub fn front_most(&self, x: f64, y: f64) -> Option<&str> {
        self.under(x, y).next()
    }
}

/// The front-most box of `boxes` under the point: [`HitIndex::from_boxes`] and one question. For a scene that is asked more
/// than once, keep the index.
pub fn hit_test(boxes: &[PaintedBox], x: f64, y: f64) -> Option<String> {
    HitIndex::from_boxes(boxes)
        .front_most(x, y)
        .map(str::to_owned)
}

/// Whether the point is in the rectangle: the left and top edges are in, the right and bottom ones are out. False for a
/// point or a rectangle that is not a number.
pub fn contains_point(r: Rect, x: f64, y: f64) -> bool {
    x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
}

/// The overlap of two rectangles; one with no width and no height at the nearer edge when they do not meet.
fn overlap(a: Rect, b: Rect) -> Rect {
    // `f64::max` and `min` pass over a NaN, which would make half of a bad rectangle the overlap; a number that is not one cuts
    // everything away (no check lets one through today, so this is for a caller that does not go through the check).
    if [a.x, a.y, a.w, a.h, b.x, b.y, b.w, b.h]
        .iter()
        .any(|v| v.is_nan())
    {
        return Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        };
    }
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (a.x + a.w).min(b.x + b.w);
    let bottom = (a.y + a.h).min(b.y + b.h);
    Rect {
        x,
        y,
        w: (right - x).max(0.0),
        h: (bottom - y).max(0.0),
    }
}
