//! The facts the gate reads from a room's tree to tier a delete (ADR-0010).

use meldframe_host::NodeFacts;
use meldframe_protocol::{Node, NodeKind};
use std::collections::HashMap;

/// The most nodes a delete may remove (the node and all its descendants) and still be a small delete, which is
/// Auto under the default policy. One node more is Ask. Today only a frame can have children (the core
/// refuses a child under a non-frame node), so no tree the room builds reaches the limit: every non-frame
/// delete is small and every frame delete is Ask. The limit guards against a tree that is not built so. Here, not in the gate, because the gate holds no file
/// state and this crate reads the tree; the number is a product default, open to the owner's override.
pub const SMALL_DELETE_MAX_NODES: usize = 10;

/// A read of a tree as it is now. Build it at submit time from `Room::nodes` of the room the ops target and
/// hand it to `Gate::submit_with`. It is a snapshot of the slice it borrows: the room can apply other ops
/// before the admitted op reaches `Room::accept`. The room then re-checks the one thing the relief rests on:
/// a delete admitted below the Delete tier is applied only if the node is still a non-frame node without
/// children, so a reused id (a leaf removed and a frame added under the same id) cannot be removed as a small
/// delete; otherwise `accept` refuses it as `forbidden` (Delete, Ask). It does not re-count the subtree, but
/// today only a frame has children, so a non-frame node is always a leaf. A node that grew between submit
/// and accept can only have grown into a frame or gained children, and both are refused.
#[derive(Debug)]
pub struct TreeFacts<'a> {
    nodes: HashMap<&'a str, &'a Node>,
    children: HashMap<&'a str, Vec<&'a str>>,
}

impl<'a> TreeFacts<'a> {
    pub fn new(tree: &'a [Node]) -> Self {
        let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
        for node in tree {
            if let Some(parent) = node.parent_id.as_deref() {
                children.entry(parent).or_default().push(&node.id);
            }
        }
        TreeFacts {
            nodes: tree.iter().map(|n| (n.id.as_str(), n)).collect(),
            children,
        }
    }

    /// The kind of the node, `None` when it is not in the tree.
    pub fn kind(&self, id: &str) -> Option<NodeKind> {
        self.nodes.get(id).map(|n| n.kind)
    }

    /// The node and its descendants, counted up to `cap` (a larger subtree reports `cap`); `None` when the
    /// node is not in the tree.
    pub fn subtree_size(&self, id: &str, cap: usize) -> Option<usize> {
        let (&root, _) = self.nodes.get_key_value(id)?;
        let (mut count, mut stack) = (0, vec![root]);
        while let Some(next) = stack.pop() {
            count += 1;
            if count >= cap {
                break;
            }
            stack.extend(self.children.get(next).into_iter().flatten().copied());
        }
        Some(count)
    }
}

impl NodeFacts for TreeFacts<'_> {
    fn delete_is_small(&self, node_id: &str) -> bool {
        self.kind(node_id).is_some_and(|k| k != NodeKind::Frame)
            && self
                .subtree_size(node_id, SMALL_DELETE_MAX_NODES + 1)
                .is_some_and(|n| n <= SMALL_DELETE_MAX_NODES)
    }
}
