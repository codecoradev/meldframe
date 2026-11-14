//! Helpers shared by the history tests.
#![allow(dead_code)]

use meldframe_history::FileState;
use meldframe_order::sib_cmp;
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

/// A file with `root > p > a > k`, built through commands so every node carries a `pos`.
pub fn base() -> FileState {
    let mut s = FileState::new();
    for (id, kind, parent) in [
        ("root", "frame", None),
        ("p", "frame", Some("root")),
        ("a", "frame", Some("p")),
        ("k", "rect", Some("a")),
    ] {
        s.apply(&add(id, kind, parent)).unwrap();
    }
    s
}

/// Ids of the children of `parent` in sibling order.
pub fn order_under(s: &FileState, parent: &str) -> Vec<String> {
    let mut v: Vec<&Node> = s
        .nodes()
        .iter()
        .filter(|n| n.parent_id.as_deref() == Some(parent))
        .collect();
    v.sort_by(|a, b| sib_cmp(a, b));
    v.into_iter().map(|n| n.id.clone()).collect()
}

/// The tree in a canonical form: list order carries no meaning, so sort by id. `pos` is included, so
/// "restored exactly" includes the sibling order.
pub fn snap(s: &FileState) -> String {
    let mut v = s.nodes().to_vec();
    v.sort_by(|a, b| a.id.cmp(&b.id));
    serde_json::to_string(&v).unwrap()
}

/// Small deterministic generator, so a failing seed can be replayed.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() as usize) % n.max(1)
    }
}
