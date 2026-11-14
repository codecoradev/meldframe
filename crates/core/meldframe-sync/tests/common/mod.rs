//! Helpers shared by the sync tests.
#![allow(dead_code)]

use meldframe_protocol::{Command, Node, NodePatch};

pub fn node(id: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent})).unwrap()
}

pub fn add(id: &str, kind: &str, parent: Option<&str>) -> Command {
    Command::AddNode {
        node: node(id, kind, parent),
        index: None,
    }
}

/// An update that sets only the fields `f` touches.
pub fn upd(id: &str, f: impl FnOnce(&mut NodePatch)) -> Command {
    let mut patch = NodePatch::default();
    f(&mut patch);
    Command::UpdateNode {
        id: id.into(),
        patch,
    }
}

pub fn remove(id: &str) -> Command {
    Command::RemoveNode { id: id.into() }
}

/// A small tree: `root > p > a > k`.
pub fn base() -> Vec<Node> {
    vec![
        node("root", "frame", None),
        node("p", "frame", Some("root")),
        node("a", "frame", Some("p")),
        node("k", "rect", Some("a")),
    ]
}

/// The tree in a canonical form (list order has no meaning, so sort by id; `pos` is included).
pub fn snap(nodes: &[Node]) -> String {
    let mut v = nodes.to_vec();
    v.sort_by(|a, b| a.id.cmp(&b.id));
    serde_json::to_string(&v).unwrap()
}

pub fn x_of(nodes: &[Node], id: &str) -> f64 {
    nodes.iter().find(|n| n.id == id).unwrap().x
}
