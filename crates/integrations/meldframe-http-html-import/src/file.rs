//! What a call needs from the file: where the root goes.

use crate::plan::Planned;
use meldframe_protocol::{Command, Node};

/// Put the root where the call says: at the origin of `parent`, else 40 px to the right of the rightmost top-level
/// node (at the top, `y` 0, and at 0 in an empty file). A root that the file already holds, because an earlier try of
/// this call wrote part of it, keeps the place it has, so the retry is the same command.
pub fn place(p: &mut Planned, parent: Option<&str>, nodes: &[Node]) {
    let root = p.root.clone();
    let Some(Command::AddNode { node, .. }) = p.plan.commands.first_mut() else {
        return;
    };
    node.parent_id = parent.map(str::to_owned);
    (node.x, node.y) = match parent {
        Some(_) => (0.0, 0.0),
        None => {
            let right = (nodes.iter().filter(|n| n.parent_id.is_none()))
                .map(|n| n.x + n.w)
                .reduce(f64::max);
            (right.map_or(0.0, |r| r + meldframe_variant::GAP), 0.0)
        }
    };
    if let Some(have) = nodes.iter().find(|n| n.id == root) {
        (node.x, node.y) = (have.x, have.y);
    }
}
