//! The geometry of drawing a node with a tool (T8 of the thin UI audit, decision 43): the box between two world points, the
//! frame the new node lands in, and what a click makes. Pure, like `drop`.

use crate::handle::MIN_SIZE;
use meldframe_check::ApplyError;
use meldframe_layout::Rect;
use meldframe_protocol::{Node, NodeKind};
use serde::Serialize;
use std::collections::HashSet;

/// The box a pointer drag from `from` to `to` draws, in world space. `size` is none for a click (both extents are 0): the
/// node then gets the default size of its kind. A drag that is flat on one side still gets [`MIN_SIZE`] there. Refused as
/// `NotFinite` for a point that is not a number.
pub fn drawn_box(from: (f64, f64), to: (f64, f64)) -> Result<DrawnBox, ApplyError> {
    if ![from.0, from.1, to.0, to.1].iter().all(|v| v.is_finite()) {
        return Err(ApplyError::NotFinite("from or to"));
    }
    let (w, h) = ((from.0 - to.0).abs(), (from.1 - to.1).abs());
    let size = (!(w == 0.0 && h == 0.0)).then(|| (w.max(MIN_SIZE), h.max(MIN_SIZE)));
    if size.is_some_and(|(w, h)| !(w.is_finite() && h.is_finite())) {
        return Err(ApplyError::NotFinite("size"));
    }
    Ok(DrawnBox {
        x: from.0.min(to.0),
        y: from.1.min(to.1),
        size,
    })
}

/// A box drawn by a drag (see [`drawn_box`]): the top left corner in world space and the size, none for a click.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawnBox {
    pub x: f64,
    pub y: f64,
    pub size: Option<(f64, f64)>,
}

/// The frame a new node goes into: the node under the press if it is a frame, else the nearest frame above it, else none (the
/// canvas). `under` is the front-most node under the press (`meldframe_render::HitIndex`). That is the pressed frame or its
/// ancestor, as the web editor chose it; `drop`'s `frame_under` asks another question (the front-most frame under the pointer
/// that is not the node dragged), so the two can differ where a rectangle lies over a frame that is not its parent.
pub fn container_for<'a>(nodes: &'a [Node], under: Option<&str>) -> Option<&'a Node> {
    let find = |id: &str| nodes.iter().find(|n| n.id == id);
    let mut seen = HashSet::new();
    let mut cur = under.and_then(find);
    while let Some(n) = cur {
        if n.kind == NodeKind::Frame {
            return Some(n);
        }
        if !seen.insert(n.id.as_str()) {
            return None;
        }
        cur = n.parent_id.as_deref().and_then(find);
    }
    None
}

/// What a draw would make, for a front end to show before the release (`App::draw_preview`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawPreview {
    /// The frame the node goes into, none for the canvas.
    pub parent: Option<String>,
    /// The box in the space of the parent (the fields of the node that is made).
    pub local: Rect,
    /// The same box on the canvas.
    pub world: Rect,
    /// A click, not a drag: the box has the default size of the kind.
    pub click: bool,
    /// The flow of an auto layout frame places the node, with the layout on the canvas: it is drawn where the flow puts it
    /// and not at `local` (the same as a node made from a menu), and `local` is only what is stored.
    pub placed_by_layout: bool,
}
