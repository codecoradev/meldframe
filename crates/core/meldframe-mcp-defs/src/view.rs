//! What the read tools say: the tree, the nodes and the changes, as JSON. Pure: no server, no IO.

use meldframe_batch::{command_name, touches};
use meldframe_host::StoredOp;
use meldframe_protocol::{Node, Variables};
use serde_json::{Value, json};
use std::collections::HashMap;

/// The last op that touched each node, from a window of the log.
pub type Last = HashMap<String, (u64, String)>;

/// For each node id, the newest of `ops` that touched it (`ops` are in sequence order).
pub fn last_touch(ops: &[StoredOp]) -> Last {
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
pub fn tree(nodes: &[Node], last: &Last, max_depth: Option<usize>) -> Vec<Value> {
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
pub fn nodes(all: &[Node], ids: &[String]) -> (Vec<Value>, Vec<String>) {
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
pub fn node(n: &Node) -> Value {
    serde_json::to_value(n).unwrap_or(Value::Null)
}

/// The ops of `ops` that touched `node_id`, as `{seq, actor, command}`.
pub fn changes_of(ops: &[StoredOp], node_id: &str) -> Vec<Value> {
    ops.iter()
        .filter(|o| touches(&o.command).iter().any(|id| id == node_id))
        .map(|o| json!({"seq": o.seq, "actor": o.actor, "command": command_name(&o.command)}))
        .collect()
}

/// Why `variables` could not answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariablesError {
    /// The collection id is not in the file.
    NoCollection,
    /// An argument is out of bounds, or `after` is not one of the variables that match.
    Invalid(String),
}

/// The collections of a file and a page of its variables, in the order they were added. `collection` and `prefix`
/// narrow the variables; at most `limit` (1 to [`crate::MAX_VARIABLES_PAGE`], default 100) are listed, after the
/// variable `after`. `next` is the id to pass as `after` for the following page, or null at the end.
pub fn variables(
    vars: &Variables,
    (collection, prefix, after): (Option<&str>, Option<&str>, Option<&str>),
    limit: Option<usize>,
) -> Result<Value, VariablesError> {
    let limit = limit.unwrap_or(100);
    let long = |s: Option<&str>, max: usize| s.is_some_and(|s| s.chars().count() > max);
    if !(1..=crate::MAX_VARIABLES_PAGE).contains(&limit)
        || long(collection, 64)
        || long(after, 64)
        || long(prefix, 256)
    {
        return Err(VariablesError::Invalid(format!(
            "limit is 1 to {}; collection and after are at most 64 characters, prefix at most 256",
            crate::MAX_VARIABLES_PAGE
        )));
    }
    if collection.is_some_and(|c| vars.collection(c).is_none()) {
        return Err(VariablesError::NoCollection);
    }
    let matching: Vec<_> = vars
        .variables
        .iter()
        .filter(|v| collection.is_none_or(|c| v.collection == c))
        .filter(|v| prefix.is_none_or(|p| v.name.starts_with(p)))
        .collect();
    let start = match after {
        None => 0,
        Some(id) => {
            1 + matching.iter().position(|v| v.id == id).ok_or_else(|| {
                VariablesError::Invalid(
                    "after is not a variable that matches this call; start again without it".into(),
                )
            })?
        }
    };
    let page: Vec<Value> = matching
        .iter()
        .skip(start)
        .take(limit)
        .map(|v| json!({"id": v.id, "collection": v.collection, "name": v.name, "type": v.kind, "values": v.values}))
        .collect();
    let next = (start + page.len() < matching.len())
        .then(|| page.last().map(|v| v["id"].clone()))
        .flatten()
        .unwrap_or(Value::Null);
    let collections: Vec<Value> = vars
        .collections
        .iter()
        .map(|c| {
            let count = vars
                .variables
                .iter()
                .filter(|v| v.collection == c.id)
                .count();
            json!({"id": c.id, "name": c.name, "modes": c.modes, "variables": count})
        })
        .collect();
    Ok(
        json!({"collections": collections, "variables": page, "total": matching.len(), "next": next}),
    )
}
