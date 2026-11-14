//! Helpers of the history property tests (a small copy of the ones the history crate's own tests use).
#![allow(dead_code)]

use meldframe_history::FileState;
use meldframe_protocol::{Command, Node};

pub fn node(id: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent})).unwrap()
}

pub fn add(id: &str, kind: &str, parent: Option<&str>) -> Command {
    Command::AddNode {
        node: node(id, kind, parent),
        index: None,
    }
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
