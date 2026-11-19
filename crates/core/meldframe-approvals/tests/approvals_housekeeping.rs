//! E2.4: the owner can list what waits, and finished approvals do not pile up in memory.

use common::*;
use meldframe_approvals::{ApprovalState, Approvals, Verdict};

mod common;

#[test]
fn pending_lists_only_the_open_approvals_of_one_workspace() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let a = ask(&mut approvals, &room, "k1", "f", 10);
    let b = ask(&mut approvals, &room, "k2", "g", 10);
    approvals.decide(&a, Verdict::Deny, "owner:o", 20).unwrap();
    let ids: Vec<&str> = approvals
        .pending("ws_1")
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(ids, [b.as_str()]);
    assert!(approvals.pending("ws_other").is_empty());
}

#[test]
fn prune_drops_finished_approvals_after_their_ttl_and_keeps_live_ones() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let denied = ask(&mut approvals, &room, "k1", "f", 10);
    approvals
        .decide(&denied, Verdict::Deny, "owner:o", 20)
        .unwrap();
    let open = ask(&mut approvals, &room, "k2", "g", 150);
    approvals.prune(10 + 2 * TTL - 1);
    assert!(
        approvals.get(&denied).is_some(),
        "a denial holds one more ttl past its time"
    );
    approvals.prune(10 + 2 * TTL);
    assert!(approvals.get(&denied).is_none());
    assert_eq!(approvals.get(&open).unwrap().state, ApprovalState::Pending);
}

#[test]
fn an_id_is_never_reused_after_a_prune() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let first = ask(&mut approvals, &room, "k1", "f", 10);
    approvals.prune(10 + 2 * TTL);
    let second = ask(&mut approvals, &room, "k1", "f", 10 + 2 * TTL + 2);
    assert_ne!(first, second);
}

#[test]
fn the_decision_log_keeps_only_the_newest_entries() {
    let (room, mut approvals) = (room(), Approvals::new(1));
    for i in 0..(Approvals::MAX_DECISIONS + 5) {
        let at = 10 * i as u64;
        ask(&mut approvals, &room, &format!("k{i}"), "f", at);
        // The next call at a later time expires it and logs the expiry.
    }
    approvals.prune(u64::MAX);
    assert_eq!(approvals.decisions().len(), Approvals::MAX_DECISIONS);
}

#[test]
fn an_answer_comes_from_another_actor_of_the_same_workspace() {
    use meldframe_approvals::{AnswerError, DecideError};
    use meldframe_host::Principal;
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    let own = approvals.answer(&who(), &id, true, 20).unwrap_err();
    assert_eq!(own, AnswerError::OwnRequest { id: id.clone() });
    let foreign = Principal::new("ws_other", "owner:x");
    let unknown = approvals.answer(&foreign, &id, true, 20).unwrap_err();
    assert_eq!(
        unknown,
        AnswerError::Decide(DecideError::Unknown { id: id.clone() })
    );
    let owner = Principal::new("ws_1", "owner:o");
    let done = approvals.answer(&owner, &id, false, 20).unwrap();
    assert_eq!(done.state, ApprovalState::Denied);
    assert_eq!(approvals.decisions()[0].by.as_deref(), Some("owner:o"));
}
