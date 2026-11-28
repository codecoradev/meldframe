//! E2.4: a frame delete is Ask. Through the server it opens an approval; the owner decides; the same call sent
//! again is admitted once, or stays refused with a stable code. Every step is in the call log.

use meldframe_approvals::{AnswerError, DecideError};
use meldframe_calls::Decision;
use meldframe_host::{Op, Principal};
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::Command;
use meldframe_server::{Server, SubmitError};

struct Painter;
impl Module for Painter {
    fn name(&self) -> &'static str {
        "painter"
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool("paint", "Change a node.", RiskTier::Change);
    }
}

fn agent() -> Principal {
    Principal::new("ws_1", "agent:a")
}

fn owner() -> Principal {
    Principal::new("ws_1", "owner:o")
}

fn frame(key: &str, id: &str) -> Op {
    let node = serde_json::from_value(serde_json::json!({"id": id, "type": "frame"})).unwrap();
    Op::new(key, Command::AddNode { node, index: None })
}

fn rm(key: &str, id: &str) -> Op {
    Op::new(key, Command::RemoveNode { id: id.into() })
}

fn server() -> Server {
    let mut s = Server::assemble_with(vec![Box::new(Painter)]);
    s.submit_tool("paint", "f1", &agent(), vec![frame("k-f", "f")])
        .unwrap();
    s
}

/// Send the delete and return the id of the approval it opened.
fn ask(s: &mut Server, key: &str) -> String {
    match s.submit_tool("paint", "f1", &agent(), vec![rm(key, "f")]) {
        Err(SubmitError::Approval(a)) => {
            assert_eq!(a.code(), "approval_required");
            s.pending_approvals("ws_1").last().unwrap().id.clone()
        }
        other => panic!("expected approval_required, got {other:?}"),
    }
}

fn nodes(s: &Server) -> usize {
    s.with_room("ws_1", "f1", |r| r.nodes().len()).unwrap()
}

#[test]
fn a_frame_delete_asks_and_writes_nothing() {
    let mut s = server();
    let id = ask(&mut s, "k1");
    assert_eq!(nodes(&s), 1);
    // Asking again while it waits names the same approval.
    assert_eq!(ask(&mut s, "k1"), id);
    assert_eq!(s.pending_approvals("ws_1").len(), 1);
    assert!(s.pending_approvals("ws_2").is_empty());
}

#[test]
fn an_approved_delete_is_admitted_once() {
    let mut s = server();
    let id = ask(&mut s, "k1");
    s.decide_approval(&owner(), &id, true).unwrap();
    let done = s
        .submit_tool("paint", "f1", &agent(), vec![rm("k1", "f")])
        .unwrap();
    assert!(done[0].written);
    assert_eq!(nodes(&s), 0);
    assert!(s.pending_approvals("ws_1").is_empty());
}

#[test]
fn a_denied_delete_stays_refused_with_its_code() {
    let mut s = server();
    let id = ask(&mut s, "k1");
    s.decide_approval(&owner(), &id, false).unwrap();
    for _ in 0..2 {
        let err = s
            .submit_tool("paint", "f1", &agent(), vec![rm("k1", "f")])
            .unwrap_err();
        assert!(matches!(err, SubmitError::Approval(_)), "{err:?}");
        assert_eq!(err.code(), "approval_denied");
    }
    assert_eq!(nodes(&s), 1);
}

#[test]
fn an_approval_that_ran_out_stays_refused_with_its_code() {
    let mut s = server().with_approval_ttl(400);
    let id = ask(&mut s, "k1");
    // Past the ttl (400 ms) and before a prune forgets it (800 ms): wide margins, so a slow machine cannot flip it.
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert_eq!(
        s.decide_approval(&owner(), &id, true).unwrap_err(),
        AnswerError::Decide(DecideError::Expired { id: id.clone() })
    );
    let err = s
        .submit_tool("paint", "f1", &agent(), vec![rm("k1", "f")])
        .unwrap_err();
    assert_eq!(err.code(), "approval_expired");
    assert_eq!(nodes(&s), 1);
}

#[test]
fn an_agent_cannot_answer_its_own_request_and_another_workspace_cannot_see_it() {
    let mut s = server();
    let id = ask(&mut s, "k1");
    let own = s.decide_approval(&agent(), &id, true).unwrap_err();
    assert_eq!(own, AnswerError::OwnRequest { id: id.clone() });
    let other = Principal::new("ws_2", "owner:x");
    let unknown = s.decide_approval(&other, &id, true).unwrap_err();
    assert_eq!(
        unknown,
        AnswerError::Decide(DecideError::Unknown { id: id.clone() })
    );
    // Nothing was decided: the delete still asks.
    assert_eq!(ask(&mut s, "k1"), id);
}

#[test]
fn a_batch_that_holds_a_delete_is_still_forbidden() {
    let mut s = server();
    let err = s
        .submit_tool(
            "paint",
            "f1",
            &agent(),
            vec![frame("k2", "g"), rm("k3", "f")],
        )
        .unwrap_err();
    assert_eq!(err.code(), "forbidden");
    assert!(s.pending_approvals("ws_1").is_empty());
}

#[test]
fn the_call_log_holds_the_ask_the_answer_and_the_admission() {
    let mut s = server();
    let id = ask(&mut s, "k1");
    s.decide_approval(&owner(), &id, true).unwrap();
    s.submit_tool("paint", "f1", &agent(), vec![rm("k1", "f")])
        .unwrap();
    let log = s.calls(10);
    let seen: Vec<_> = log
        .iter()
        .map(|e| {
            (
                e.actor.as_str(),
                e.tool.as_str(),
                e.decision,
                e.outcome.as_str(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("agent:a", "paint", Some(Decision::Allow), "ok"),
            ("agent:a", "paint", Some(Decision::Ask), "approval_required"),
            ("owner:o", "decide_approval", Some(Decision::Allow), "ok"),
            ("agent:a", "paint", Some(Decision::Allow), "ok"),
        ]
    );
    assert_eq!(log[1].risk, Some(RiskTier::Delete));
}

#[test]
fn a_delete_of_a_node_that_is_not_in_the_file_asks_nobody() {
    let mut s = server();
    let err = s
        .submit_tool("paint", "f1", &agent(), vec![rm("k1", "ghost")])
        .unwrap_err();
    assert_eq!(err.code(), "forbidden");
    assert!(s.pending_approvals("ws_1").is_empty());
}

#[test]
fn an_approved_op_that_the_room_then_refuses_is_spent() {
    let mut s = server();
    // `f` was written at seq 1, so an `if_seq` of 0 is stale: the gate asks, and the room refuses the retry.
    let stale = || Op {
        if_seq: Some(0),
        ..rm("k1", "f")
    };
    let id = match s.submit_tool("paint", "f1", &agent(), vec![stale()]) {
        Err(SubmitError::Approval(a)) => a.id().unwrap().to_string(),
        other => panic!("{other:?}"),
    };
    s.decide_approval(&owner(), &id, true).unwrap();
    let err = s
        .submit_tool("paint", "f1", &agent(), vec![stale()])
        .unwrap_err();
    assert_eq!(err.code(), "conflict");
    // The approval is gone: the same op asks again, under a new id.
    let again = s
        .submit_tool("paint", "f1", &agent(), vec![stale()])
        .unwrap_err();
    assert_eq!(again.code(), "approval_required");
    assert_ne!(s.pending_approvals("ws_1")[0].id, id);
}

#[test]
fn an_actor_cannot_hold_more_open_approvals_than_the_cap() {
    let mut s = server();
    let many = meldframe_approvals::Approvals::MAX_OPEN_PER_ACTOR;
    let frames: Vec<Op> = (0..=many)
        .map(|i| frame(&format!("a{i}"), &format!("n{i}")))
        .collect();
    s.submit_tool("paint", "f1", &agent(), frames).unwrap();
    for i in 0..many {
        let r = s.submit_tool(
            "paint",
            "f1",
            &agent(),
            vec![rm(&format!("d{i}"), &format!("n{i}"))],
        );
        assert_eq!(r.unwrap_err().code(), "approval_required");
    }
    let over = s
        .submit_tool(
            "paint",
            "f1",
            &agent(),
            vec![rm("d-over", &format!("n{many}"))],
        )
        .unwrap_err();
    assert_eq!(over.code(), "approval_limit");
    let last = s.calls(1).pop().unwrap();
    assert_eq!(
        (last.decision, last.outcome.as_str()),
        (Some(Decision::Deny), "approval_limit")
    );
    assert_eq!(s.pending_approvals("ws_1").len(), many);
}
