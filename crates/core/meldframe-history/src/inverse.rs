//! The inverse of a command: another valid command that undoes it, worked out from the state before
//! the command is applied. Undo travels the same path as an ordinary edit, so it can be sent to peers.

use crate::file_state::FileState;
use crate::patch_fields::patch_fields;
use meldframe_check::position;
use meldframe_protocol::{Command, Node, NodePatch};
use std::collections::HashSet;

fn some_pos(n: &Node) -> Option<String> {
    (!n.pos.is_empty()).then(|| n.pos.clone())
}

/// A patch that holds the old value of every field `p` touches.
fn inverse_patch(old: &Node, p: &NodePatch) -> NodePatch {
    let mut r = NodePatch::default();
    if p.image.is_some() {
        // The old value: `None` (no image) becomes the patch `null`.
        r.image = Some(old.image.clone());
    }
    macro_rules! copy {
        ($($f:ident),*) => { $( if p.$f.is_some() { r.$f = Some(old.$f.clone()); } )* };
    }
    patch_fields!(copy);
    r
}

/// The node `id` and all its descendants.
fn subtree(nodes: &[Node], id: &str) -> Vec<Node> {
    let mut set: HashSet<&str> = HashSet::from([id]);
    let mut grew = true;
    while grew {
        grew = false;
        for n in nodes {
            let child = n.parent_id.as_deref().is_some_and(|p| set.contains(p));
            if child && set.insert(n.id.as_str()) {
                grew = true;
            }
        }
    }
    nodes
        .iter()
        .filter(|n| set.contains(n.id.as_str()))
        .cloned()
        .collect()
}

impl FileState {
    /// The commands that undo `cmd` against the current tree (call before applying `cmd`; `cmd` must
    /// already be validated).
    pub(crate) fn invert(&self, cmd: &Command) -> Vec<Command> {
        match cmd {
            Command::UpdateNode { id, patch } => {
                let old = &self.nodes[position(&self.nodes, id).expect("checked")];
                vec![Command::UpdateNode {
                    id: id.clone(),
                    patch: inverse_patch(old, patch),
                }]
            }
            Command::AddNode { node, .. } => vec![Command::RemoveNode {
                id: node.id.clone(),
            }],
            Command::Reparent { id, .. } => {
                let old = &self.nodes[position(&self.nodes, id).expect("checked")];
                vec![Command::Reparent {
                    id: id.clone(),
                    parent_id: old.parent_id.clone(),
                    pos: some_pos(old),
                }]
            }
            Command::Reorder { id, .. } => {
                let old = &self.nodes[position(&self.nodes, id).expect("checked")];
                vec![Command::Reorder {
                    id: id.clone(),
                    before: None,
                    after: None,
                    pos: some_pos(old),
                }]
            }
            // Each node keeps its own `pos`, so the sibling order comes back exactly. If another sibling
            // holds that `pos` by then, the apply rule puts the node right behind it.
            Command::RemoveNode { id } => subtree(&self.nodes, id)
                .into_iter()
                .map(|node| Command::AddNode { node, index: None })
                .collect(),
        }
    }
}
