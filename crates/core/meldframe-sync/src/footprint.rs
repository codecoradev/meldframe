//! The footprint of a command: what it reads and what it writes, as `(node, field)` pairs.
//!
//! The inverse of an undo or redo is worked out from the optimistic view. A pending op is a premise of
//! it when the op wrote something the inverse reads or writes: if the server refuses that op, the value
//! or the node was never there. The rules are conservative:
//!
//! - read `Field(f)`: the value of field `f`; written by `UpdateNode` of `f`, by `Reparent` and `Reorder`
//!   (`parentId`, `pos`) and by `AddNode` and `RemoveNode`;
//! - read `EXISTS`: that the node exists (a parent, an anchor, a node that is removed); written only by
//!   `AddNode` and `RemoveNode` of that node;
//! - read `ALL`: the whole content of a node (the snapshot in `AddNode`); written by every write to it.
//!
//! An undo or redo also reads what it writes, because it implies that the op it reverses took effect.

use meldframe_protocol::{Command, Node, NodePatch};
use std::collections::HashSet;

/// Marks "this node exists".
const EXISTS: &str = "\u{0}exists";
/// Marks "the whole node".
const ALL: &str = "\u{0}all";
/// The field name of a write that creates or removes a node.
const WHOLE: &str = "*";

pub(crate) type Pairs = Vec<(String, String)>;

fn pair(id: &str, field: &str) -> (String, String) {
    (id.to_string(), field.to_string())
}

fn patch_fields(p: &NodePatch) -> Vec<String> {
    match serde_json::to_value(p) {
        Ok(serde_json::Value::Object(m)) => m.keys().cloned().collect(),
        _ => vec![],
    }
}

/// The node `id` and everything under it.
fn subtree_ids(nodes: &[Node], id: &str) -> Vec<String> {
    let mut out = vec![id.to_string()];
    let mut seen: HashSet<&str> = HashSet::from([id]);
    let mut i = 0;
    while i < out.len() {
        let cur = out[i].clone();
        for n in nodes {
            if n.parent_id.as_deref() == Some(cur.as_str()) && seen.insert(n.id.as_str()) {
                out.push(n.id.clone());
            }
        }
        i += 1;
    }
    out
}

/// `(reads, writes)` of `c` against the view `nodes` as it is before `c` is applied.
pub(crate) fn footprint(nodes: &[Node], c: &Command) -> (Pairs, Pairs) {
    match c {
        Command::UpdateNode { id, patch } => {
            let w: Pairs = patch_fields(patch).iter().map(|f| pair(id, f)).collect();
            (w.clone(), w)
        }
        Command::Reparent { id, parent_id, .. } => {
            let w = vec![pair(id, "parentId"), pair(id, "pos")];
            let mut r = w.clone();
            r.push(pair(id, EXISTS));
            if let Some(p) = parent_id {
                r.push(pair(p, EXISTS));
            }
            (r, w)
        }
        Command::Reorder {
            id, before, after, ..
        } => {
            let w = vec![pair(id, "pos")];
            let mut r = w.clone();
            r.push(pair(id, EXISTS));
            for a in before.iter().chain(after.iter()) {
                r.push(pair(a, "pos"));
                r.push(pair(a, EXISTS));
            }
            (r, w)
        }
        Command::AddNode { node, .. } => {
            let mut r = vec![pair(&node.id, ALL)];
            if let Some(p) = &node.parent_id {
                r.push(pair(p, EXISTS));
            }
            (r, vec![pair(&node.id, WHOLE)])
        }
        Command::RemoveNode { id } => {
            let ids = subtree_ids(nodes, id);
            (
                ids.iter().map(|n| pair(n, EXISTS)).collect(),
                ids.iter().map(|n| pair(n, WHOLE)).collect(),
            )
        }
        // A variable command is refused before it gets here.
        _ => (Vec::new(), Vec::new()),
    }
}

/// Is the write `w` a premise of the read `r` (on the same node)?
pub(crate) fn premise(r: &str, w: &str) -> bool {
    w == WHOLE || r == ALL || r == w
}
