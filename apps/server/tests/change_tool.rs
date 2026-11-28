//! The gate against tools of each tier through the composed server: what a Change tool and a Read tool may submit, and
//! what an unknown tool gets. Moved from `apps/server/tests/assemble.rs`, which is at the size ceiling of its crate.

use meldframe_host::{Op, Principal, Refusal};
use meldframe_module::{Module, Policy, Registrar, RiskTier};
use meldframe_protocol::Command;
use meldframe_server::{Server, SubmitError};

fn who() -> Principal {
    Principal::new("ws_dev", "agent:test")
}

fn add(key: &str) -> Op {
    let node = serde_json::from_value(serde_json::json!({"id": "n1", "type": "frame"})).unwrap();
    Op::new(key, Command::AddNode { node, index: None })
}

struct Painter;
impl Module for Painter {
    fn name(&self) -> &'static str {
        "painter"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Paint a node.", RiskTier::Change);
    }
}

#[test]
fn a_change_tool_gets_its_change_op_admitted_under_the_default_policy() {
    let mut server = Server::assemble_with(vec![Box::new(Painter)]);
    let report = server
        .submit_tool("paint", "file_dev", &who(), vec![add("k1")])
        .unwrap();
    assert_eq!(report.len(), 1);
}

#[test]
fn a_change_tool_cannot_submit_a_delete() {
    let mut server = Server::assemble_with(vec![Box::new(Painter)]);
    let rm = Op::new("k2", Command::RemoveNode { id: "n1".into() });
    let err = server
        .submit_tool("paint", "file_dev", &who(), vec![rm])
        .unwrap_err();
    assert!(
        matches!(
            err,
            SubmitError::Refused(Refusal::Forbidden {
                tier: RiskTier::Delete,
                ..
            })
        ),
        "{err:?}"
    );
}

#[test]
fn an_unknown_tool_is_reported_with_the_tier_of_what_it_tried() {
    let mut server = Server::assemble();
    let err = server
        .submit_tool("no_such_tool", "file_dev", &who(), vec![add("k1")])
        .unwrap_err();
    assert!(
        matches!(
            err,
            SubmitError::Refused(Refusal::Forbidden {
                tier: RiskTier::Change,
                policy: Policy::Deny
            })
        ),
        "{err:?}"
    );
}

struct Looker;
impl Module for Looker {
    fn name(&self) -> &'static str {
        "looker"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("look", "Look at a node.", RiskTier::Read);
    }
}

/// A server whose room holds frame `f` with one rect `r` in it.
fn server_with_nodes() -> Server {
    let mut server = Server::assemble_with(vec![Box::new(Painter), Box::new(Looker)]);
    let node = |id: &str, kind: &str, parent: Option<&str>| {
        serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent}))
            .unwrap()
    };
    let ops = vec![
        Op::new(
            "a1",
            Command::AddNode {
                node: node("f", "frame", None),
                index: None,
            },
        ),
        Op::new(
            "a2",
            Command::AddNode {
                node: node("r", "rect", Some("f")),
                index: None,
            },
        ),
    ];
    server
        .submit_tool("paint", "file_dev", &who(), ops)
        .unwrap();
    server
}

fn rm(id: &str) -> Op {
    Op::new("k9", Command::RemoveNode { id: id.into() })
}

#[test]
fn a_change_tool_gets_a_small_delete_of_an_existing_node_admitted() {
    let report = server_with_nodes()
        .submit_tool("paint", "file_dev", &who(), vec![rm("r")])
        .unwrap();
    assert_eq!(report.len(), 1);
}

#[test]
fn a_read_tool_cannot_submit_even_a_small_delete() {
    let err = server_with_nodes()
        .submit_tool("look", "file_dev", &who(), vec![rm("r")])
        .unwrap_err();
    assert!(
        matches!(
            err,
            SubmitError::Refused(Refusal::Forbidden {
                policy: Policy::Deny,
                ..
            })
        ),
        "{err:?}"
    );
}

#[test]
fn a_change_tool_asks_the_owner_before_it_deletes_a_frame() {
    let err = server_with_nodes()
        .submit_tool("paint", "file_dev", &who(), vec![rm("f")])
        .unwrap_err();
    assert_eq!(err.code(), "approval_required", "{err:?}");
}
