//! Sibling order: the effective order of siblings is `(pos, id)`, never the position in the node list.

use meldframe_protocol::Node;
use std::cmp::Ordering;
use std::collections::HashMap;

/// Sibling comparator: `(pos, id)`. A node without a `pos` (an old file not yet migrated) keeps list
/// order (so the sort must be stable) and comes before every node that has one.
pub fn sib_cmp(a: &Node, b: &Node) -> Ordering {
    a.pos.cmp(&b.pos).then_with(|| {
        if a.pos.is_empty() {
            Ordering::Equal
        } else {
            a.id.cmp(&b.id)
        }
    })
}

/// The children of each parent (indices into `nodes`) ordered by `(pos, id)`. Key `None` is the root level.
pub fn children_map(nodes: &[Node]) -> HashMap<Option<&str>, Vec<usize>> {
    let mut kids: HashMap<Option<&str>, Vec<usize>> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        kids.entry(n.parent_id.as_deref()).or_default().push(i);
    }
    for v in kids.values_mut() {
        let sorted = v
            .windows(2)
            .all(|w| sib_cmp(&nodes[w[0]], &nodes[w[1]]) != Ordering::Greater);
        if !sorted {
            v.sort_by(|&a, &b| sib_cmp(&nodes[a], &nodes[b]));
        }
    }
    kids
}
