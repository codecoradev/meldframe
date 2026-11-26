//! The node tree as layout reads it: children grouped by parent in sibling order.

use meldframe_order::sib_cmp;
use meldframe_protocol::{LayoutMode, Node, NodeKind};
use std::collections::HashMap;

/// A frame that arranges its children (row or column); other frames leave children at their own offsets.
pub fn is_layout_frame(n: &Node) -> bool {
    n.kind == NodeKind::Frame && n.layout != LayoutMode::None
}

/// Indexes into `nodes` of each parent's children in sibling order (`meldframe_order::sib_cmp`). The key `None` holds
/// the roots.
pub(super) fn children_by_parent(nodes: &[Node]) -> HashMap<Option<&str>, Vec<usize>> {
    let mut kids: HashMap<Option<&str>, Vec<usize>> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        kids.entry(n.parent_id.as_deref()).or_default().push(i);
    }
    for v in kids.values_mut() {
        v.sort_by(|&a, &b| sib_cmp(&nodes[a], &nodes[b]));
    }
    kids
}
