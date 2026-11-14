//! Helpers shared by the host tests.
#![allow(dead_code)]

use meldframe_host::{Gate, Op, Permit, Principal, Refusal, RoomError};
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

/// An op and the permit the gate gives it, for `actor` in workspace `ws_1`.
pub fn admit(actor: &str, op: Op) -> (Op, Permit) {
    let who = Principal::new("ws_1", actor);
    let a = Gate::new()
        .submit(&who, vec![op])
        .unwrap()
        .admitted
        .remove(0);
    (a.op, a.permit)
}

pub fn refusal(e: RoomError) -> Refusal {
    match e {
        RoomError::Refused(r) => r,
        other => panic!("expected a refusal, got {other}"),
    }
}
