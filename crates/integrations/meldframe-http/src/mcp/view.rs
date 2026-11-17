//! What the read tools say: the tree, the nodes and the changes, as JSON. Pure: no server, no IO.

use meldframe_batch::{command_name, touches};
use meldframe_host::StoredOp;
use meldframe_protocol::Node;
use serde_json::{Value, json};
use std::collections::HashMap;

/// The last op that touched each node, from a window of the log.
pub(super) type Last = HashMap<String, (u64, String)>;

/// For each node id, the newest of `ops` that touched it (`ops` are in sequence order).
pub(super) fn last_touch(ops: &[StoredOp]) -> Last {
    let mut last = Last::new();
    for op in ops {
        for id in touches(&op.command) {
            last.insert(id, (op.seq, op.actor.clone()));
        }
    }
    last
}

fn kind(n: &Node) -> Value {
    serde_json::to_value(n.kind).unwrap_or(Value::Null)
}

/// The nodes as a forest: roots first, children under their parent in sibling order `(pos, id)`. A node whose
/// parent is not in the file is listed as a root, so no node is lost. `max_depth` stops the descent (0 lists
/// only the roots).
pub(super) fn tree(nodes: &[Node], last: &Last, max_depth: Option<usize>) -> Vec<Value> {
    let ids: std::collections::HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut kids: HashMap<Option<&str>, Vec<&Node>> = HashMap::new();
    for n in nodes {
        let parent = n.parent_id.as_deref().filter(|p| ids.contains(p));
        kids.entry(parent).or_default().push(n);
    }
    for list in kids.values_mut() {
        list.sort_by(|a, b| (&a.pos, &a.id).cmp(&(&b.pos, &b.id)));
    }
    let roots = kids.get(&None).cloned().unwrap_or_default();
    roots
        .iter()
        .map(|n| entry(n, &kids, last, max_depth, 0))
        .collect()
}

fn entry(
    n: &Node,
    kids: &HashMap<Option<&str>, Vec<&Node>>,
    last: &Last,
    max_depth: Option<usize>,
    depth: usize,
) -> Value {
    let mut v = json!({"id": n.id, "name": n.name, "kind": kind(n),
        "x": n.x, "y": n.y, "w": n.w, "h": n.h});
    if let Some((seq, actor)) = last.get(&n.id) {
        v["lastSeq"] = json!(seq);
        v["lastActor"] = json!(actor);
    }
    let children = kids
        .get(&Some(n.id.as_str()))
        .map_or(&[][..], Vec::as_slice);
    let below = max_depth.is_none_or(|m| depth < m);
    v["childCount"] = json!(children.len());
    if below {
        v["children"] = Value::Array(
            children
                .iter()
                .map(|c| entry(c, kids, last, max_depth, depth + 1))
                .collect(),
        );
    }
    v
}

/// The full data of the nodes with these ids, and the ids that are not in the file.
pub(super) fn nodes(all: &[Node], ids: &[String]) -> (Vec<Value>, Vec<String>) {
    let mut found = Vec::new();
    let mut missing = Vec::new();
    for id in ids {
        match all.iter().find(|n| n.id == *id) {
            Some(n) => found.push(node(n)),
            None => missing.push(id.clone()),
        }
    }
    (found, missing)
}

/// One node's full data.
pub(super) fn node(n: &Node) -> Value {
    serde_json::to_value(n).unwrap_or(Value::Null)
}

/// The ops of `ops` that touched `node_id`, as `{seq, actor, command}`.
pub(super) fn changes_of(ops: &[StoredOp], node_id: &str) -> Vec<Value> {
    ops.iter()
        .filter(|o| touches(&o.command).iter().any(|id| id == node_id))
        .map(|o| json!({"seq": o.seq, "actor": o.actor, "command": command_name(&o.command)}))
        .collect()
}
