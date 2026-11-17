//! Helpers shared by the gate tests.
#![allow(dead_code)]

use meldframe_host::Op;
use meldframe_protocol::{Command, Node};

pub fn node(id: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent})).unwrap()
}

pub fn add(id: &str) -> Command {
    Command::AddNode {
        node: node(id, "frame", None),
        index: None,
    }
}

pub fn remove(id: &str) -> Command {
    Command::RemoveNode { id: id.into() }
}

pub fn op(key: &str, command: Command) -> Op {
    Op::new(key, command)
}
