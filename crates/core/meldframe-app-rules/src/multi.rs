//! A drag of several selected nodes in a laid-out file: the commands of the drop and what to show while the pointer is
//! down. Pure, like `drop`, which it builds on: a drop of one node is `drop`'s, to the bit.
//!
//! The rules (the owner's decisions of 21 Nov 2026):
//!
//! - The nodes that move are the selected ones that have no selected ancestor (a node and its selected descendants are
//!   one node: the descendants follow it).
//! - The children of auto layout frames among them are put in the flow; the others move by the drag (`UpdateNode` of `x`
//!   and `y`, as `MoveBy` does).
//! - When all the flow children are children of the same frame they move as one block, keeping the order they have in
//!   the flow, to the place the pointer names:
//!   - with the pointer inside that frame, the block goes to the slot at the pointer: before the first other flow child
//!     whose middle (along the main axis) is past the pointer, else last. A `Reorder` for each, so no stored `x` or `y` of
//!     a flow child is written. A block that is already there, in one piece, makes no command;
//!   - with the pointer over another auto layout frame (not one of the dragged nodes or inside one), the block goes into
//!     the flow of that frame at the pointer, in order (a `Reparent` each, at keys between the neighbours);
//!   - anywhere else it leaves the flow, each node keeping the place it was drawn at plus the drag, a `fill` side becoming
//!     fixed (as `drop` does for one node).
//! - Flow children of more than one frame: they stay where they are, and only the others move. Nothing writes a stored
//!   position of a flow child.
//!
//! `drop_many_preview` answers the same question without applying anything: the slot (as `drop_slot`), and which of the
//! dragged nodes are to be drawn following the pointer (the ones that go with the drag; a block that goes into a slot is
//! shown by the line, and a flow child that stays is not drawn moving). Both read one decision (`decide`), so what is
//! shown is what a release does. A drag of one node is exactly `drop_commands` and `drop_slot`.

use crate::drop::{
    Drag, Slot, contains, drop_commands, drop_slot, flow_siblings, index_at, moved, out_of_flow,
    slot_bounds, slot_line,
};
use meldframe_check::ApplyError;
use meldframe_layout::{Rect, is_layout_frame};
use meldframe_order::fracindex;
use meldframe_order::sib_cmp;
use meldframe_protocol::{Command, Node, NodeKind};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// A finished drag of a selection: the selected ids, where the pointer was let go (world space) and how far it travelled.
#[derive(Debug, Clone, Copy)]
pub struct DragMany<'a> {
    pub ids: &'a [String],
    pub at: (f64, f64),
    pub by: (f64, f64),
}

/// What a front end shows while the pointer is down: where the block would land, and which nodes follow the pointer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Preview {
    /// The slot the flow children would land in; none when they land in no flow.
    pub slot: Option<Slot>,
    /// The dragged nodes (without the descendants that follow them) that are drawn moving with the pointer.
    pub follow: Vec<String>,
}

/// Where the block of flow children goes.
enum Block<'a> {
    /// No flow child moves as a block (none is selected, or they are children of more than one frame).
    None,
    /// Within its own frame at the pointer.
    Reorder(&'a Node),
    /// Into the flow of another frame at the pointer.
    Into(&'a Node),
    /// Out of the flow: to the frame under the pointer (no layout), or to the page.
    Out(Option<&'a Node>),
}

struct Decision<'a> {
    /// The flow children that move as a block, in the order of the flow.
    block: Vec<&'a Node>,
    aim: Block<'a>,
    /// The nodes that move by the drag.
    free: Vec<&'a Node>,
}

/// What the rules look things up by, built once per call: every node by id, and every laid-out box by id. A lookup in the
/// list would make the rules quadratic in the selection.
struct Index<'a> {
    by_id: HashMap<&'a str, &'a Node>,
    boxes: HashMap<&'a str, Rect>,
}

impl<'a> Index<'a> {
    fn new(nodes: &'a [Node], rects: &'a [(String, Rect)]) -> Self {
        Index {
            by_id: nodes.iter().map(|n| (n.id.as_str(), n)).collect(),
            boxes: rects.iter().map(|(i, r)| (i.as_str(), *r)).collect(),
        }
    }

    fn parent(&self, n: &Node) -> Option<&'a Node> {
        n.parent_id
            .as_deref()
            .and_then(|p| self.by_id.get(p).copied())
    }
}

/// The commands of a drop of the selection `d.ids`, as one step; none when nothing changes. `rects` as in
/// [`drop_commands`]. An id that is not a node is skipped (someone deleted it while the drag went on); `NotFound` when none
/// of them is one. `NotFinite` for a number that is not one.
pub fn drop_many_commands(
    nodes: &[Node],
    rects: &[(String, Rect)],
    d: &DragMany,
) -> Result<Vec<Command>, ApplyError> {
    if ![d.at.0, d.at.1, d.by.0, d.by.1]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err(ApplyError::NotFinite("x, y, dx or dy"));
    }
    let tops = top_nodes(nodes, d.ids)?;
    if let [one] = tops[..] {
        return drop_commands(
            nodes,
            rects,
            &Drag {
                id: &one.id,
                at: d.at,
                by: d.by,
            },
        );
    }
    let ix = Index::new(nodes, rects);
    let dec = decide(&ix, nodes, rects, tops, d.at);
    let mut out = match dec.aim {
        Block::None => Vec::new(),
        Block::Reorder(frame) => reorder_block(nodes, rects, &dec.block, frame, d.at),
        Block::Into(frame) => into_flow_block(nodes, rects, &dec.block, frame, d.at)?,
        Block::Out(to) => {
            let origin = to
                .and_then(|t| ix.boxes.get(t.id.as_str()))
                .map_or((0.0, 0.0), |r| (r.x, r.y));
            dec.block
                .iter()
                .flat_map(|n| {
                    let mine = ix.boxes[n.id.as_str()];
                    out_of_flow(n, to, mine, origin, d.by)
                })
                .collect()
        }
    };
    out.extend(dec.free.iter().map(|n| moved(n, d.by)));
    Ok(out)
}

/// What to show for a drag of `d.ids`; none when none of the ids is a node and for a number that is not one.
pub fn drop_many_preview(
    nodes: &[Node],
    rects: &[(String, Rect)],
    d: &DragMany,
) -> Option<Preview> {
    if ![d.at.0, d.at.1].iter().all(|v| v.is_finite()) {
        return None;
    }
    let tops = top_nodes(nodes, d.ids).ok()?;
    if let [one] = tops[..] {
        let slot = drop_slot(
            nodes,
            rects,
            &Drag {
                id: &one.id,
                at: d.at,
                by: d.by,
            },
        );
        let follow = if slot.is_some() {
            Vec::new()
        } else {
            vec![one.id.clone()]
        };
        return Some(Preview { slot, follow });
    }
    let ix = Index::new(nodes, rects);
    let dec = decide(&ix, nodes, rects, tops, d.at);
    let slot = match dec.aim {
        Block::Reorder(frame) | Block::Into(frame) => {
            let skip: HashSet<&str> = dec.block.iter().map(|n| n.id.as_str()).collect();
            let sibs = flow_siblings(nodes, rects, frame, &skip);
            let index = index_at(frame, &sibs, d.at);
            Some(Slot {
                frame: frame.id.clone(),
                index,
                line: slot_line(rects, frame, &sibs, index),
            })
        }
        Block::None | Block::Out(_) => None,
    };
    let leaving = matches!(dec.aim, Block::Out(_));
    let follow = dec
        .block
        .iter()
        .filter(|_| leaving)
        .chain(dec.free.iter())
        .map(|n| n.id.clone())
        .collect();
    Some(Preview { slot, follow })
}

/// The selected nodes that have no selected ancestor, in the order of `ids`, each once. An id that is not a node is skipped
/// (a node someone deleted while the drag went on); `NotFound` when none of them is one. A node and its selected
/// descendants are one node: the descendants follow it. Linear in the file and the selection.
pub fn top_nodes<'a>(nodes: &'a [Node], ids: &[String]) -> Result<Vec<&'a Node>, ApplyError> {
    let by_id: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut chosen: Vec<&Node> = Vec::new();
    for id in ids {
        if let Some(&n) = by_id.get(id.as_str())
            && seen.insert(n.id.as_str())
        {
            chosen.push(n);
        }
    }
    if chosen.is_empty() {
        return match ids.first() {
            Some(id) => Err(ApplyError::NotFound(id.clone())),
            None => Ok(chosen),
        };
    }
    let under_a_chosen = |n: &Node| {
        let mut cur = n.parent_id.as_deref().and_then(|p| by_id.get(p).copied());
        // A parent chain longer than the file is a cycle: stop.
        for _ in 0..=nodes.len() {
            match cur {
                Some(p) if seen.contains(p.id.as_str()) => return true,
                Some(p) => cur = p.parent_id.as_deref().and_then(|q| by_id.get(q).copied()),
                None => return false,
            }
        }
        false
    };
    Ok(chosen
        .iter()
        .copied()
        .filter(|n| !under_a_chosen(n))
        .collect())
}

/// What a drop of these top nodes does, before the commands are worked out.
fn decide<'a>(
    ix: &Index<'a>,
    nodes: &'a [Node],
    rects: &[(String, Rect)],
    tops: Vec<&'a Node>,
    at: (f64, f64),
) -> Decision<'a> {
    let (mut flow, mut free) = (Vec::new(), Vec::new());
    for &n in &tops {
        let in_flow = ix.boxes.contains_key(n.id.as_str())
            && !n.absolute
            && ix.parent(n).is_some_and(is_layout_frame);
        if in_flow { flow.push(n) } else { free.push(n) }
    }
    flow.sort_by(|a, b| sib_cmp(a, b));
    let frame = flow.first().and_then(|n| ix.parent(n));
    let one_frame = frame.is_some_and(|f| {
        flow.iter()
            .all(|n| n.parent_id.as_deref() == Some(f.id.as_str()))
    });
    let (Some(frame), true) = (frame, one_frame) else {
        // Flow children of different frames stay where they are.
        return Decision {
            block: Vec::new(),
            aim: Block::None,
            free,
        };
    };
    let aim = if ix
        .boxes
        .get(frame.id.as_str())
        .is_some_and(|r| contains(*r, at))
    {
        Block::Reorder(frame)
    } else {
        match frame_under(ix, nodes, rects, &tops, at) {
            Some(t) if is_layout_frame(t) && t.id != frame.id => Block::Into(t),
            under => Block::Out(under),
        }
    };
    Decision {
        block: flow,
        aim,
        free,
    }
}

/// The front-most frame under the pointer that is not one of the dragged nodes or inside one (the last box drawn wins).
fn frame_under<'a>(
    ix: &Index<'a>,
    nodes: &[Node],
    rects: &[(String, Rect)],
    tops: &[&Node],
    at: (f64, f64),
) -> Option<&'a Node> {
    let top_ids: HashSet<&str> = tops.iter().map(|t| t.id.as_str()).collect();
    let inside_a_top = |n: &Node| {
        let mut cur = Some(n);
        for _ in 0..=nodes.len() {
            match cur {
                Some(c) if top_ids.contains(c.id.as_str()) => return true,
                Some(c) => cur = ix.parent(c),
                None => return false,
            }
        }
        false
    };
    rects.iter().rev().find_map(|(id, r)| {
        let n = ix
            .by_id
            .get(id.as_str())
            .copied()
            .filter(|n| n.kind == NodeKind::Frame && contains(*r, at))?;
        (!inside_a_top(n)).then_some(n)
    })
}

/// The block put at the pointer within its own frame, in one piece; nothing when it is there already.
fn reorder_block(
    nodes: &[Node],
    rects: &[(String, Rect)],
    block: &[&Node],
    frame: &Node,
    at: (f64, f64),
) -> Vec<Command> {
    let skip: HashSet<&str> = block.iter().map(|n| n.id.as_str()).collect();
    let sibs = flow_siblings(nodes, rects, frame, &skip);
    let idx = index_at(frame, &sibs, at);
    // Already in one piece at that place: the others before the first of the block are `idx`, and it is not split.
    let all = flow_siblings(nodes, rects, frame, &HashSet::new());
    let first = all.iter().position(|(n, _)| n.id == block[0].id);
    let whole = first.is_some_and(|f| {
        f == idx
            && all[f..]
                .iter()
                .take(block.len())
                .map(|(n, _)| n.id.as_str())
                .eq(block.iter().map(|b| b.id.as_str()))
    });
    if whole {
        return Vec::new();
    }
    let mut out = vec![match sibs.get(idx) {
        Some((s, _)) => Command::Reorder {
            id: block[0].id.clone(),
            before: Some(s.id.clone()),
            after: None,
            pos: None,
        },
        None => Command::Reorder {
            id: block[0].id.clone(),
            before: None,
            after: sibs.last().map(|(s, _)| s.id.clone()),
            pos: None,
        },
    }];
    for pair in block.windows(2) {
        out.push(Command::Reorder {
            id: pair[1].id.clone(),
            before: None,
            after: Some(pair[0].id.clone()),
            pos: None,
        });
    }
    out
}

/// The block moved into the flow of `frame` at the pointer, in order, at keys between the neighbours.
fn into_flow_block(
    nodes: &[Node],
    rects: &[(String, Rect)],
    block: &[&Node],
    frame: &Node,
    at: (f64, f64),
) -> Result<Vec<Command>, ApplyError> {
    let skip: HashSet<&str> = block.iter().map(|n| n.id.as_str()).collect();
    let sibs = flow_siblings(nodes, rects, frame, &skip);
    let idx = index_at(frame, &sibs, at);
    let (lo, hi) = slot_bounds(nodes, frame, &skip, &sibs, idx);
    let mut prev = lo.map(str::to_string);
    let mut out = Vec::new();
    for n in block {
        let pos = fracindex::between(prev.as_deref(), hi)
            .map_err(|e| ApplyError::InvalidPos(e.to_string()))?;
        prev = Some(pos.clone());
        out.push(Command::Reparent {
            id: n.id.clone(),
            parent_id: Some(frame.id.clone()),
            pos: Some(pos),
        });
    }
    Ok(out)
}
