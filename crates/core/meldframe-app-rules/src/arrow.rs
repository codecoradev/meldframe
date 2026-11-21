//! What an arrow key does to the selection in a laid-out file: the selected children of an auto layout frame move one
//! place along its main axis. A pure function of the nodes, the selection and the key, like `drop`: no state, no clock,
//! nothing of the front end, so a desktop front end that reports the key the same way gets the same document.
//!
//! The rules (the owner's decisions of 21 Nov 2026):
//!
//! - The key along the main axis of the frame (up and down in a column, left and right in a row) moves a selected child
//!   one place towards the start (up, left) or the end (down, right) of the flow. It is a `Reorder`; the stored `x` and `y`
//!   of a flow child are ignored by the layout and left alone. The key across the axis does nothing.
//! - Several selected children of the same frame move together, each one place, the way a list does: a child whose
//!   neighbour in that direction is not selected swaps with it, the others stay, so the group keeps its order and stops at
//!   the end of the flow. The children are worked out from the far end, so every command reads the order the one before it
//!   left.
//! - The selection may hold other nodes (free ones, absolute ones): they are left alone, since an arrow key is not a move
//!   of a free node (`MoveBy` is). Flow children of more than one frame in the selection: nothing moves, as the frames
//!   disagree on what one place is.
//!
//! The commands are for the one history step the app makes of them. They need no laid-out boxes: the order of the flow is
//! the order of the siblings.

use meldframe_app_model::Direction;
use meldframe_layout::is_layout_frame;
use meldframe_order::children_map;
use meldframe_protocol::{Command, LayoutMode, Node};
use std::collections::HashSet;

/// The commands of the arrow key `key` for the nodes `selected`; none when nothing moves.
pub fn arrow_commands(nodes: &[Node], selected: &[String], key: Direction) -> Vec<Command> {
    let chosen: HashSet<&str> = selected.iter().map(String::as_str).collect();
    let by_id = |id: &str| nodes.iter().find(|n| n.id == id);
    // The frames whose flow has a selected child.
    let mut frames: Vec<&Node> = Vec::new();
    for id in selected {
        let Some(n) = by_id(id) else { continue };
        let Some(parent) = n.parent_id.as_deref().and_then(by_id) else {
            continue;
        };
        if !n.absolute && is_layout_frame(parent) && !frames.iter().any(|f| f.id == parent.id) {
            frames.push(parent);
        }
    }
    let [frame] = frames[..] else {
        return Vec::new();
    };
    // Towards the start (-1) or the end (+1) of the flow; the key across the main axis is no move.
    let later = match (frame.layout, key) {
        (LayoutMode::Column, Direction::Up) | (LayoutMode::Row, Direction::Left) => false,
        (LayoutMode::Column, Direction::Down) | (LayoutMode::Row, Direction::Right) => true,
        _ => return Vec::new(),
    };
    let kids = children_map(nodes);
    let mut flow: Vec<&str> = kids
        .get(&Some(frame.id.as_str()))
        .into_iter()
        .flatten()
        .map(|&i| &nodes[i])
        .filter(|k| !k.absolute)
        .map(|k| k.id.as_str())
        .collect();
    let mut out = Vec::new();
    // Start at the end the group moves towards, so a swap never reads a place that has not been settled.
    let order: Vec<usize> = if later {
        (0..flow.len().saturating_sub(1)).rev().collect()
    } else {
        (1..flow.len()).collect()
    };
    for i in order {
        let (me, other) = if later { (i, i + 1) } else { (i, i - 1) };
        if chosen.contains(flow[me]) && !chosen.contains(flow[other]) {
            let (id, neighbour) = (flow[me].to_string(), flow[other].to_string());
            out.push(Command::Reorder {
                id,
                before: (!later).then(|| neighbour.clone()),
                after: later.then_some(neighbour),
                pos: None,
            });
            flow.swap(me, other);
        }
    }
    out
}
