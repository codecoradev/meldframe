//! The rectangles a camera fits to: what is on the canvas, and what is selected, in world space. A painted box holds its offset
//! inside its parent, so both read the world rectangles of [`HitIndex`], which sums the offsets once.

use crate::capture::union;
use crate::hit::{HitIndex, overlap};
use meldframe_layout::Rect;
use std::collections::HashSet;

/// The rectangle around everything that is on the canvas, or `None` for an empty scene: every box at its own rectangle, with
/// what a clipping ancestor cuts away left out (a box that is cut away whole adds nothing), so a child that sticks out of a
/// frame that does not clip counts and one that sticks out of a frame that does is cut.
pub fn content_extent(index: &HitIndex) -> Option<Rect> {
    index
        .placed()
        .filter_map(|(_, rect, clip)| match clip {
            None => Some(rect),
            Some(c) => Some(overlap(rect, c)).filter(|r| r.w > 0.0 && r.h > 0.0),
        })
        .reduce(union)
}

/// The rectangle around the boxes of `ids`, or `None` when none of them is a box of the index. An id that is not a box adds
/// nothing, and what a clipping ancestor cuts away stays in it: a selection is where the nodes are.
pub fn extent_of(index: &HitIndex, ids: &[String]) -> Option<Rect> {
    let ids: HashSet<&str> = ids.iter().map(String::as_str).collect();
    index
        .placed()
        .filter(|(id, ..)| ids.contains(id))
        .map(|(_, rect, _)| rect)
        .reduce(union)
}
