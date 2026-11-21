//! The node tree as layout reads it: children grouped by parent in sibling order.

use meldframe_protocol::{LayoutMode, Node, NodeKind};
use std::cmp::Ordering;
use std::collections::HashMap;

/// A frame that arranges its children (row or column); other frames leave children at their own offsets.
pub fn is_layout_frame(n: &Node) -> bool {
    n.kind == NodeKind::Frame && n.layout != LayoutMode::None
}

/// Sibling order is (position, id). A node without a position (an older document not yet migrated)
/// keeps list order and sorts before positioned nodes.
fn sibling_order(a: &Node, b: &Node) -> Ordering {
    a.pos.cmp(&b.pos).then_with(|| {
        if a.pos.is_empty() {
            Ordering::Equal
        } else {
            a.id.cmp(&b.id)
        }
    })
}

/// Indexes into `nodes` of each parent's children in sibling order. The key `None` holds the roots.
pub(super) fn children_by_parent(nodes: &[Node]) -> HashMap<Option<&str>, Vec<usize>> {
    let mut kids: HashMap<Option<&str>, Vec<usize>> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        kids.entry(n.parent_id.as_deref()).or_default().push(i);
    }
    for v in kids.values_mut() {
        v.sort_by(|&a, &b| sibling_order(&nodes[a], &nodes[b]));
    }
    kids
}
