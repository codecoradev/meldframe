//! Helpers shared by the approval tests.
#![allow(dead_code)]

use meldframe_approvals::{ApprovalGate, ApprovalRefusal, Approvals, Verdict};
use meldframe_host::{Gate, MemoryStore, Op, Principal, Report, Room};
use meldframe_module::default_policy;
use meldframe_protocol::Command;
use meldframe_rooms::TreeFacts;

pub const TTL: u64 = 100;

pub fn who() -> Principal {
    Principal::new("ws_1", "agent:a")
}

pub fn add(key: &str, id: &str, kind: &str, parent: Option<&str>) -> Op {
    let node =
        serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent}))
            .unwrap();
    Op::new(key, Command::AddNode { node, index: None })
}

pub fn rm(key: &str, id: &str) -> Op {
    Op::new(key, Command::RemoveNode { id: id.into() })
}

/// A room holding frames `f` and `g`.
pub fn room() -> Room<MemoryStore> {
    let mut room = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
    let ops = vec![
        add("k-f", "f", "frame", None),
        add("k-g", "g", "frame", None),
    ];
    for adm in Gate::new().submit(&who(), ops).unwrap().admitted {
        room.accept(adm).unwrap();
    }
    room
}

/// The default policy, on a gate that can honour an approval.
pub fn dflt(approvals: &Approvals) -> ApprovalGate {
    approvals.gate(|_, tier| default_policy(tier))
}

pub fn submit(
    approvals: &mut Approvals,
    room: &Room<MemoryStore>,
    op: Op,
    at: u64,
) -> Result<Report, ApprovalRefusal> {
    let mut gate = dflt(approvals);
    approvals.submit(
        &mut gate,
        &who(),
        "file_1",
        op,
        &TreeFacts::new(room.nodes()),
        at,
    )
}

/// Submit a frame delete and return the approval id it opened.
pub fn ask(
    approvals: &mut Approvals,
    room: &Room<MemoryStore>,
    key: &str,
    id: &str,
    at: u64,
) -> String {
    match submit(approvals, room, rm(key, id), at) {
        Err(ApprovalRefusal::Required { id }) => id,
        other => panic!("expected approval_required, got {other:?}"),
    }
}

/// Submit as an explicit principal and gate (for the quota, workspace and tier cases).
pub fn submit_as(
    approvals: &mut Approvals,
    gate: &mut ApprovalGate,
    who: &Principal,
    room: &Room<MemoryStore>,
    op: Op,
    at: u64,
) -> Result<Report, ApprovalRefusal> {
    approvals.submit(gate, who, "file_1", op, &TreeFacts::new(room.nodes()), at)
}

pub fn approve(approvals: &mut Approvals, id: &str) {
    approvals
        .decide(id, Verdict::Approve, "owner:o", 20)
        .unwrap();
}
