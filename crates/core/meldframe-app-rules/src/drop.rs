//! Where a dragged node lands in a laid-out file: the commands of a drop, worked out from the nodes, the laid-out boxes
//! and the pointer. A pure function of its arguments: no state, no clock, and nothing of the front end in it, so a
//! web page and a desktop front end that report a drop the same way get the same document.
//!
//! The rules (the owner's decisions of 21 Nov 2026 on interaction in a laid-out frame):
//!
//! - A child of an auto layout frame, dropped with the pointer anywhere inside that frame, is put in the flow at the
//!   pointer: before the first flow sibling whose middle (along the main axis of the frame) is past the pointer, else
//!   last. It is a `Reorder`; the stored `x` and `y` of a flow child are ignored by the layout and left alone. Inside its
//!   own parent a drop is always a reorder: a sibling that is a frame is never a place to nest into (to nest, the
//!   pointer has to leave the parent first, or a later, explicit gesture does it). A pointer exactly on the middle of a
//!   sibling is not past it.
//! - Dropped with the pointer outside its own parent, it leaves the parent for the front-most frame under the pointer
//!   (not the node or anything inside it): into that frame's flow at the pointer when the frame has auto layout (a
//!   `Reparent` at a position between the neighbours, `x` and `y` left alone), else to the place the node was drawn at
//!   plus the drag (`x` and `y` in the new frame's space), and a side sized `fill` becomes fixed at the size it was drawn
//!   at (a fill means nothing outside a flow). Over no frame, it goes to the page the same way.
//! - A node that is not in a flow (in a free frame, absolute, or at the top of the page) moves by the drag, except that
//!   a node of a free frame dropped on an auto layout frame goes into its flow. A Frame at the top of the page is never
//!   nested into another by a drag: it only moves.
//!
//! A node the layout holds no box of (its parent is not in the file, or it is in a cycle) is moved by the drag.
//!
//! `drop_slot` answers the same question without applying anything: where in a flow the node would go (the frame, the
//! index among its other flow children and a line across the gap), for a front end that shows it while the pointer is
//! down. Both read one decision (`aim`), so the preview and the drop cannot disagree.
//!
//! `Properties::position_locked` is advisory in this slice: the panel is told the position comes from the layout, and a
//! front end that disables the fields keeps a person from typing one; `Move` and `MoveBy` are not guarded here.

use meldframe_check::ApplyError;
use meldframe_layout::{Rect, is_layout_frame};
use meldframe_order::{children_map, fracindex, sib_cmp};
use meldframe_protocol::{Command, LayoutMode, Node, NodeKind, NodePatch, SizeMode};
use serde::Serialize;
use std::collections::HashMap;

/// A finished drag: the node, where the pointer was let go (world space) and how far it travelled since the press.
#[derive(Debug, Clone, Copy)]
pub struct Drag<'a> {
    pub id: &'a str,
    pub at: (f64, f64),
    pub by: (f64, f64),
}

/// The commands of a drop, to apply as one step; none when the node stays where it is. `rects` are the boxes of the
/// layout in world space, in drawing order (`meldframe_layout::compute_rects`): a node it holds no box of is moved by the
/// drag. Refused as `NotFound` for an unknown node and `NotFinite` for a number that is not one.
pub fn drop_commands(
    nodes: &[Node],
    rects: &[(String, Rect)],
    d: &Drag,
) -> Result<Vec<Command>, ApplyError> {
    if ![d.at.0, d.at.1, d.by.0, d.by.1]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err(ApplyError::NotFinite("x, y, dx or dy"));
    }
    let n = find(nodes, d.id).ok_or_else(|| ApplyError::NotFound(d.id.to_string()))?;
    let Some(mine) = box_of(rects, d.id) else {
        return Ok(vec![moved(n, d.by)]);
    };
    match aim(nodes, rects, n, d) {
        Aim::Reorder(frame) => Ok(reorder(nodes, rects, n, frame, d.at)),
        Aim::Into(frame) => into_flow(nodes, rects, n, frame, d.at),
        Aim::Out(to) => Ok(out_of_flow(n, to, mine, rects, d.by)),
        Aim::Move => Ok(vec![moved(n, d.by)]),
    }
}

/// Where in a flow a drop would put the node: the frame, how many of its other flow children come before the node, and a
/// line across the gap it lands in (zero thick, in world space) for a front end to draw. `drop_commands` and this read
/// the same decision (`aim`), so a preview shows what a release does. None when the drop goes into no flow (it moves
/// the node, or takes it out of its flow), for an unknown node, and for a number that is not one.
pub fn drop_slot(nodes: &[Node], rects: &[(String, Rect)], d: &Drag) -> Option<Slot> {
    if ![d.at.0, d.at.1].iter().all(|v| v.is_finite()) {
        return None;
    }
    let n = find(nodes, d.id)?;
    box_of(rects, d.id)?;
    match aim(nodes, rects, n, d) {
        Aim::Reorder(frame) | Aim::Into(frame) => {
            let sibs = flow_siblings(nodes, rects, frame, &n.id);
            let index = index_at(frame, &sibs, d.at);
            let line = slot_line(rects, frame, &sibs, index);
            Some(Slot {
                frame: frame.id.clone(),
                index,
                line,
            })
        }
        Aim::Out(_) | Aim::Move => None,
    }
}

/// A place in a flow: see [`drop_slot`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Slot {
    pub frame: String,
    pub index: usize,
    pub line: Rect,
}

/// What a drop does, before the commands are worked out.
enum Aim<'a> {
    /// Put in the flow of its own parent at the pointer.
    Reorder(&'a Node),
    /// Put in the flow of another frame at the pointer.
    Into(&'a Node),
    /// Out of its flow: to the frame under the pointer (no layout) or to the page.
    Out(Option<&'a Node>),
    /// Moved by the drag.
    Move,
}

/// The decision of a drop, for a node the layout holds a box of.
fn aim<'a>(nodes: &'a [Node], rects: &[(String, Rect)], n: &'a Node, d: &Drag) -> Aim<'a> {
    // A node whose parent is not in the file gets no box (the layout does not place it) and never gets here, so a
    // parent that is named here is in the file.
    let parent = n.parent_id.as_deref().and_then(|p| find(nodes, p));
    let flows = !n.absolute && parent.is_some_and(is_layout_frame);
    // Inside its own flow parent a drop is a reorder, whatever frame a sibling is.
    if flows {
        let frame = parent.expect("flows");
        if box_of(rects, &frame.id).is_some_and(|r| contains(r, d.at)) {
            return Aim::Reorder(frame);
        }
    }
    let under = frame_under(nodes, rects, d);
    let same = under.map(|t| t.id.as_str()) == parent.map(|p| p.id.as_str());
    let top_frame = parent.is_none() && n.kind == NodeKind::Frame;
    match under {
        Some(t) if is_layout_frame(t) && !same && (flows || !(n.absolute || top_frame)) => {
            Aim::Into(t)
        }
        _ if flows => Aim::Out(under),
        _ => Aim::Move,
    }
}

/// The line of a slot: across the main axis of the frame, in the middle of the gap between the neighbours (on the edge
/// of the only neighbour at either end, at the start of the inside of the frame when there is none), as wide as the neighbour.
fn slot_line(rects: &[(String, Rect)], frame: &Node, sibs: &[(&Node, f64)], index: usize) -> Rect {
    let row = frame.layout == LayoutMode::Row;
    let of = |i: usize| sibs.get(i).and_then(|(k, _)| box_of(rects, &k.id));
    let (prev, next) = (index.checked_sub(1).and_then(of), of(index));
    // The inside of the frame, for a flow with no neighbour.
    let pad = frame.pad();
    let outer = box_of(rects, &frame.id).unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    });
    let mine = Rect {
        x: outer.x + pad.left,
        y: outer.y + pad.top,
        w: (outer.w - pad.left - pad.right).max(0.0),
        h: (outer.h - pad.top - pad.bottom).max(0.0),
    };
    // (start, length) of a box along the main axis and across it.
    let main = |r: Rect| if row { (r.x, r.w) } else { (r.y, r.h) };
    let cross = |r: Rect| if row { (r.y, r.h) } else { (r.x, r.w) };
    let at = match (prev, next) {
        (Some(p), Some(n)) => (main(p).0 + main(p).1 + main(n).0) / 2.0,
        (None, Some(n)) => main(n).0,
        (Some(p), None) => main(p).0 + main(p).1,
        (None, None) => main(mine).0,
    };
    let (from, len) = cross(next.or(prev).unwrap_or(mine));
    if row {
        Rect {
            x: at,
            y: from,
            w: 0.0,
            h: len,
        }
    } else {
        Rect {
            x: from,
            y: at,
            w: len,
            h: 0.0,
        }
    }
}

fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
    nodes.iter().find(|n| n.id == id)
}

fn box_of(rects: &[(String, Rect)], id: &str) -> Option<Rect> {
    rects.iter().find(|(i, _)| i == id).map(|(_, r)| *r)
}

/// The stored position moved by the drag.
fn moved(n: &Node, by: (f64, f64)) -> Command {
    update(
        &n.id,
        NodePatch {
            x: Some(n.x + by.0),
            y: Some(n.y + by.1),
            ..Default::default()
        },
    )
}

fn update(id: &str, patch: NodePatch) -> Command {
    Command::UpdateNode {
        id: id.to_string(),
        patch,
    }
}

/// Whether `id` is `start` or one of its ancestors.
fn is_self_or_above(nodes: &[Node], id: &str, start: &Node) -> bool {
    let mut cur = Some(start);
    for _ in 0..=nodes.len() {
        match cur {
            Some(n) if n.id == id => return true,
            Some(n) => cur = n.parent_id.as_deref().and_then(|p| find(nodes, p)),
            None => return false,
        }
    }
    false
}

/// Whether the point is in the box (the left and top edges are in, the right and bottom ones are out).
fn contains(r: Rect, (x, y): (f64, f64)) -> bool {
    x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
}

/// The front-most frame under the pointer that is not the dragged node or inside it (the last box drawn wins).
fn frame_under<'a>(nodes: &'a [Node], rects: &[(String, Rect)], d: &Drag) -> Option<&'a Node> {
    // Indexed by id: a lookup for every box of the file would make this quadratic.
    let by_id: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    rects.iter().rev().find_map(|(id, r)| {
        let n = by_id
            .get(id.as_str())
            .copied()
            .filter(|n| n.kind == NodeKind::Frame && contains(*r, d.at))?;
        (!is_self_or_above(nodes, d.id, n)).then_some(n)
    })
}

/// The flow children of `frame` other than `id`, in sibling order, with the middle of their box on the main axis.
fn flow_siblings<'a>(
    nodes: &'a [Node],
    rects: &[(String, Rect)],
    frame: &Node,
    id: &str,
) -> Vec<(&'a Node, f64)> {
    let row = frame.layout == LayoutMode::Row;
    let kids = children_map(nodes);
    let boxes: HashMap<&str, Rect> = rects.iter().map(|(i, r)| (i.as_str(), *r)).collect();
    let mut out = Vec::new();
    for &i in kids.get(&Some(frame.id.as_str())).into_iter().flatten() {
        let k = &nodes[i];
        if k.id == id || k.absolute {
            continue;
        }
        if let Some(r) = boxes.get(k.id.as_str()).copied() {
            out.push((
                k,
                if row {
                    r.x + r.w / 2.0
                } else {
                    r.y + r.h / 2.0
                },
            ));
        }
    }
    out
}

/// How many of the flow siblings the pointer is past: the place it puts a node in the flow.
fn index_at(frame: &Node, siblings: &[(&Node, f64)], at: (f64, f64)) -> usize {
    let p = if frame.layout == LayoutMode::Row {
        at.0
    } else {
        at.1
    };
    siblings.iter().filter(|(_, mid)| *mid < p).count()
}

/// A node of the flow put at the pointer within its own frame; nothing when it is already there.
fn reorder(
    nodes: &[Node],
    rects: &[(String, Rect)],
    n: &Node,
    frame: &Node,
    at: (f64, f64),
) -> Vec<Command> {
    let sibs = flow_siblings(nodes, rects, frame, &n.id);
    let idx = index_at(frame, &sibs, at);
    let now = sibs.iter().filter(|(s, _)| sib_cmp(s, n).is_lt()).count();
    if idx == now {
        return Vec::new();
    }
    let (mut before, mut after) = (None, None);
    match sibs.get(idx) {
        Some((s, _)) => before = Some(s.id.clone()),
        None => after = sibs.last().map(|(s, _)| s.id.clone()),
    }
    vec![Command::Reorder {
        id: n.id.clone(),
        before,
        after,
        pos: None,
    }]
}

/// The node moved into the flow of `frame` at the pointer, at a position between its neighbours.
fn into_flow(
    nodes: &[Node],
    rects: &[(String, Rect)],
    n: &Node,
    frame: &Node,
    at: (f64, f64),
) -> Result<Vec<Command>, ApplyError> {
    let sibs = flow_siblings(nodes, rects, frame, &n.id);
    let idx = index_at(frame, &sibs, at);
    // Every child counts for the bounds of the key, the absolute ones too: the key is unique among them.
    let keys: Vec<&str> = nodes
        .iter()
        .filter(|k| k.parent_id.as_deref() == Some(frame.id.as_str()) && k.id != n.id)
        .map(|k| k.pos.as_str())
        .collect();
    let below = |p: &str| keys.iter().copied().filter(|k| *k < p).max();
    let above = |p: &str| keys.iter().copied().filter(|k| *k > p).min();
    let (lo, hi) = match (idx.checked_sub(1).and_then(|i| sibs.get(i)), sibs.get(idx)) {
        (_, Some((s, _))) => (below(&s.pos), Some(s.pos.as_str())),
        (Some((s, _)), None) => (Some(s.pos.as_str()), above(&s.pos)),
        (None, None) => (keys.iter().copied().max(), None),
    };
    let pos = fracindex::between(lo, hi).map_err(|e| ApplyError::InvalidPos(e.to_string()))?;
    Ok(vec![Command::Reparent {
        id: n.id.clone(),
        parent_id: Some(frame.id.clone()),
        pos: Some(pos),
    }])
}

/// A node of a flow taken to a frame with no layout, or to the page: it keeps the place it was drawn at plus the
/// drag, and a `fill` side becomes fixed at the size it was drawn at.
fn out_of_flow(
    n: &Node,
    to: Option<&Node>,
    mine: Rect,
    rects: &[(String, Rect)],
    by: (f64, f64),
) -> Vec<Command> {
    let origin = to
        .and_then(|t| box_of(rects, &t.id))
        .map_or((0.0, 0.0), |r| (r.x, r.y));
    let mut patch = NodePatch {
        x: Some(mine.x + by.0 - origin.0),
        y: Some(mine.y + by.1 - origin.1),
        ..Default::default()
    };
    if n.sw == SizeMode::Fill {
        (patch.sw, patch.w) = (Some(SizeMode::Fixed), Some(mine.w));
    }
    if n.sh == SizeMode::Fill {
        (patch.sh, patch.h) = (Some(SizeMode::Fixed), Some(mine.h));
    }
    vec![
        Command::Reparent {
            id: n.id.clone(),
            parent_id: to.map(|t| t.id.clone()),
            pos: None,
        },
        update(&n.id, patch),
    ]
}
