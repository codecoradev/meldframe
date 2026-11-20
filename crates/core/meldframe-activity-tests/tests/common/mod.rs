//! Helpers of the reducer tests: ops, headers and a file to run a revert on.
#![allow(dead_code)]

use meldframe_activity::{Activity, RawHeader, Review};
use meldframe_history::FileState;
use meldframe_protocol::{Command, Node, NodePatch};
use meldframe_revert::LoggedOp;

pub const ME: &str = "person:ana";

pub fn activity() -> Activity {
    Activity::new(ME)
}

pub fn node(id: &str, name: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(
        serde_json::json!({"id": id, "name": name, "type": kind, "parentId": parent}),
    )
    .unwrap()
}

/// A rectangle (the commands of the reducer tests never need a frame but for the root of `file_before`).
pub fn add(id: &str, name: &str, parent: Option<&str>) -> Command {
    Command::AddNode {
        node: node(id, name, "rect", parent),
        index: None,
    }
}

pub fn set_x(id: &str, x: f64) -> Command {
    let patch = NodePatch {
        x: Some(x),
        ..Default::default()
    };
    Command::UpdateNode {
        id: id.into(),
        patch,
    }
}

pub fn remove(id: &str) -> Command {
    Command::RemoveNode { id: id.into() }
}

pub fn op(seq: u64, actor: &str, key: &str, command: Command) -> LoggedOp {
    LoggedOp {
        seq,
        actor: actor.into(),
        key: key.into(),
        command,
    }
}

/// A header of a batch of `commands` commands, all written, with `rev` and the server time `at_ms`.
pub fn header(n: u32, rev: u64, actor: &str, prefix: &str, commands: u32, at_ms: u64) -> RawHeader {
    RawHeader {
        n,
        rev,
        actor: actor.into(),
        key_prefix: prefix.into(),
        tool: "apply_commands".into(),
        commands,
        written: commands,
        atomic: false,
        first_seq: u64::from(n),
        last_seq: u64::from(n),
        at_ms,
        note: None,
        review: Review::Unreviewed,
        reviewed_by: None,
        reviewed_at_ms: None,
    }
}

/// `root` and the nodes of `ops`, as the file the ops were written on.
pub fn file_before() -> Vec<Node> {
    let mut s = FileState::new();
    let root = Command::AddNode {
        node: node("root", "Root", "frame", None),
        index: None,
    };
    for c in [root, add("a", "A", Some("root"))] {
        s.apply(&c).unwrap();
    }
    s.nodes().to_vec()
}
