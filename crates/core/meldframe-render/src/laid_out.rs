//! `resolve_laid_out`: the painted boxes with the geometry of the layout core (hug, fill, auto layout).

use crate::{Paint, PaintedBox, resolve_with};
use meldframe_layout::{Rect, TextMeasurer, compute_rects_indexed};
use meldframe_protocol::{Node, NodeKind, Variables};
use meldframe_variables::ModeChoice;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

/// Like [`resolve_with`], with the box of each node as the layout core gives it: a `fill` or `hug` size, a child of an
/// auto layout frame at the place the flow gives it. Everything but the geometry is what `resolve_with` paints, in the
/// same order.
///
/// `x` and `y` stay the offset inside the parent, as in [`PaintedBox`], so a reader of the boxes needs no change: the
/// offset is the layout's box minus the layout's box of the parent. A node the layout core does not place (the children
/// of a node that is not a frame, and the nodes of a cycle) keeps the offset and size it stores, relative to where its
/// parent was laid out, and so stays visible. A node whose parent is missing is a root for the layout too, as it is for
/// `resolve_with`.
///
/// A file with no auto layout, no hug and no fill gives the boxes of `resolve_with` to the bit. The measurer answers for
/// hug text; it needs no variables, because no field the layout reads can be bound.
pub fn resolve_laid_out(
    nodes: &[Node],
    vars: &Variables,
    choice: &ModeChoice,
    measurer: &dyn TextMeasurer,
) -> Vec<PaintedBox> {
    let mut boxes = resolve_with(nodes, vars, choice);
    let nodes = with_missing_parents_as_roots(nodes);
    let rects: HashMap<&str, Rect> = compute_rects_indexed(&nodes, measurer)
        .into_iter()
        .map(|(i, r)| (nodes[i].id.as_str(), r))
        .collect();
    let kinds: HashMap<&str, NodeKind> = nodes.iter().map(|n| (n.id.as_str(), n.kind)).collect();
    // Where each box ended up on the canvas, parents first (the order of `boxes`).
    let mut origin: HashMap<String, (f64, f64)> = HashMap::with_capacity(boxes.len());
    for b in &mut boxes {
        let (px, py) = b
            .parent
            .as_deref()
            .and_then(|p| origin.get(p))
            .copied()
            .unwrap_or((0.0, 0.0));
        let (wx, wy) = match rects.get(b.id.as_str()) {
            Some(r) => {
                let (x, y) = (offset(r.x, px, b.x), offset(r.y, py, b.y));
                (b.x, b.y, b.w, b.h) = (x, y, r.w, r.h);
                (r.x, r.y)
            }
            None => (px + b.x, py + b.y),
        };
        if kinds.get(b.id.as_str()) == Some(&NodeKind::Ellipse) && b.paint == Paint::Rect {
            b.radius = b.w.min(b.h) / 2.0;
        }
        origin.insert(b.id.clone(), (wx, wy));
    }
    boxes
}

/// `world - parent`, or `stored` when that is the same number up to the rounding of the subtraction: a free frame places
/// its child at `parent + stored`, and a person's `10.3` must not come back as `10.299999999999997`.
fn offset(world: f64, parent: f64, stored: f64) -> f64 {
    let d = world - parent;
    if (d - stored).abs() <= 1e-9 * (1.0 + stored.abs()) {
        stored
    } else {
        d
    }
}

/// The layout core starts from the nodes without a parent; `resolve_with` also starts from those whose parent is not
/// in the file. The nodes are copied only when the file has such a node.
fn with_missing_parents_as_roots(nodes: &[Node]) -> Cow<'_, [Node]> {
    let ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let orphan = |n: &Node| n.parent_id.as_deref().is_some_and(|p| !ids.contains(p));
    if !nodes.iter().any(orphan) {
        return Cow::Borrowed(nodes);
    }
    Cow::Owned(
        nodes
            .iter()
            .cloned()
            .map(|mut n| {
                if orphan(&n) {
                    n.parent_id = None;
                }
                n
            })
            .collect(),
    )
}
