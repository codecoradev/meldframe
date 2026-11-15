//! E2.4: a call with policy Ask opens a pending approval; the same call, retried after the owner approved,
//! is admitted once. A denied or expired approval stays refused with its own code.

use common::*;
use meldframe_approvals::{ApprovalRefusal, ApprovalState, Approvals, DecideError, Verdict};
use meldframe_host::{Gate, Principal, Refusal};
use meldframe_module::{Policy, RiskTier};
use meldframe_rooms::{ActivityKind, TreeFacts};

mod common;

#[test]
fn a_frame_delete_opens_a_pending_approval() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    let a = approvals.get(&id).unwrap();
    assert_eq!(a.state, ApprovalState::Pending);
    assert_eq!((a.actor.as_str(), a.file.as_str()), ("agent:a", "file_1"));
    assert_eq!(
        (a.key.as_str(), a.kind, a.node_id.as_str()),
        ("k1", ActivityKind::RemoveNode, "f")
    );
    assert_eq!((a.created_at, a.expires_at), (10, 10 + TTL));
    assert_eq!(
        approvals
            .find("ws_1", "file_1", "agent:a", "k1")
            .map(|a| a.id.as_str()),
        Some(id.as_str())
    );
}

#[test]
fn resubmitting_while_pending_returns_the_same_approval() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let first = ask(&mut approvals, &room, "k1", "f", 10);
    assert_eq!(ask(&mut approvals, &room, "k1", "f", 20), first);
}

#[test]
fn an_approved_retry_is_admitted_with_the_delete_tier() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    approvals
        .decide(&id, Verdict::Approve, "owner:o", 20)
        .unwrap();
    let report = submit(&mut approvals, &room, rm("k1", "f"), 30).unwrap();
    assert_eq!(report.admitted.len(), 1);
    assert_eq!(report.admitted[0].permit().tier(), RiskTier::Delete);
    assert_eq!(report.admitted[0].permit().actor(), "agent:a");
}

#[test]
fn an_approval_is_single_use() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    approvals
        .decide(&id, Verdict::Approve, "owner:o", 20)
        .unwrap();
    submit(&mut approvals, &room, rm("k1", "f"), 30).unwrap();
    assert_eq!(approvals.get(&id).unwrap().state, ApprovalState::Consumed);
    // The second use is refused and asks again under a new id.
    let again = ask(&mut approvals, &room, "k1", "f", 40);
    assert_ne!(again, id);
}

#[test]
fn an_approval_does_not_cover_another_command_under_the_same_key() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    approvals
        .decide(&id, Verdict::Approve, "owner:o", 20)
        .unwrap();
    let other = ask(&mut approvals, &room, "k1", "g", 30);
    assert_ne!(other, id);
    assert_eq!(approvals.get(&id).unwrap().state, ApprovalState::Approved);
}

#[test]
fn an_approval_does_not_cover_another_actor_or_file() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    approvals
        .decide(&id, Verdict::Approve, "owner:o", 20)
        .unwrap();
    let facts = TreeFacts::new(room.nodes());
    let other_actor = Principal::new("ws_1", "agent:b");
    let mut g = dflt(&approvals);
    let r = approvals.submit(&mut g, &other_actor, "file_1", rm("k1", "f"), &facts, 30);
    assert!(matches!(r, Err(ApprovalRefusal::Required { .. })));
    let r = approvals.submit(&mut g, &who(), "file_2", rm("k1", "f"), &facts, 30);
    assert!(matches!(r, Err(ApprovalRefusal::Required { .. })));
    assert_eq!(approvals.get(&id).unwrap().state, ApprovalState::Approved);
}

#[test]
fn a_denied_approval_stays_refused() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    approvals.decide(&id, Verdict::Deny, "owner:o", 20).unwrap();
    for at in [30, 40] {
        let r = submit(&mut approvals, &room, rm("k1", "f"), at);
        assert_eq!(r.unwrap_err(), ApprovalRefusal::Denied { id: id.clone() });
    }
}

#[test]
fn a_pending_approval_expires_and_cannot_be_approved_late() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    let late = approvals.decide(&id, Verdict::Approve, "owner:o", 10 + TTL);
    assert_eq!(late.unwrap_err(), DecideError::Expired { id: id.clone() });
    let r = submit(&mut approvals, &room, rm("k1", "f"), 10 + TTL + 1);
    assert_eq!(r.unwrap_err(), ApprovalRefusal::Expired { id: id.clone() });
    assert_eq!(approvals.get(&id).unwrap().state, ApprovalState::Expired);
}

#[test]
fn an_approved_approval_expires_too() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    approvals
        .decide(&id, Verdict::Approve, "owner:o", 20)
        .unwrap();
    let r = submit(&mut approvals, &room, rm("k1", "f"), 10 + TTL);
    assert_eq!(r.unwrap_err(), ApprovalRefusal::Expired { id });
}

#[test]
fn a_decision_is_final() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    approvals.decide(&id, Verdict::Deny, "owner:o", 20).unwrap();
    let r = approvals.decide(&id, Verdict::Approve, "owner:o", 21);
    assert_eq!(
        r.unwrap_err(),
        DecideError::NotPending {
            id: id.clone(),
            state: ApprovalState::Denied
        }
    );
    assert_eq!(
        approvals
            .decide("apr_missing", Verdict::Approve, "owner:o", 21)
            .unwrap_err(),
        DecideError::Unknown {
            id: "apr_missing".into()
        }
    );
}

#[test]
fn decisions_are_recorded_with_who_and_when() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let a = ask(&mut approvals, &room, "k1", "f", 10);
    let b = ask(&mut approvals, &room, "k2", "g", 11);
    let c = ask(&mut approvals, &room, "k3", "g", 12);
    approvals
        .decide(&a, Verdict::Approve, "owner:o", 20)
        .unwrap();
    approvals.decide(&b, Verdict::Deny, "owner:p", 21).unwrap();
    let _ = submit(&mut approvals, &room, rm("k3", "g"), 12 + TTL);
    let log: Vec<_> = approvals
        .decisions()
        .iter()
        .map(|d| (d.approval.as_str(), d.state, d.by.as_deref(), d.at))
        .collect();
    assert_eq!(
        log,
        [
            (a.as_str(), ApprovalState::Approved, Some("owner:o"), 20),
            (b.as_str(), ApprovalState::Denied, Some("owner:p"), 21),
            // Past the expiry, the approved-but-unused one and the pending one run out.
            (a.as_str(), ApprovalState::Expired, None, 12 + TTL),
            (c.as_str(), ApprovalState::Expired, None, 12 + TTL),
        ]
    );
    let d = &approvals.decisions()[0];
    assert_eq!(
        (d.actor.as_str(), d.key.as_str(), d.kind, d.node_id.as_str()),
        ("agent:a", "k1", ActivityKind::RemoveNode, "f")
    );
}

#[test]
fn without_ask_nothing_changes() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    // A policy of Auto admits the delete with no approval; Deny stays forbidden and opens none.
    let facts = TreeFacts::new(room.nodes());
    let mut auto = approvals.gate(|_, _| Policy::Auto);
    let r = approvals.submit(&mut auto, &who(), "file_1", rm("k1", "f"), &facts, 10);
    assert_eq!(r.unwrap().admitted.len(), 1);
    let mut deny = approvals.gate(|_, _| Policy::Deny);
    let r = approvals.submit(&mut deny, &who(), "file_1", rm("k2", "f"), &facts, 10);
    assert_eq!(
        r.unwrap_err(),
        ApprovalRefusal::Refused(Refusal::Forbidden {
            tier: RiskTier::Delete,
            policy: Policy::Deny
        })
    );
    assert!(approvals.find("ws_1", "file_1", "agent:a", "k2").is_none());
    // The plain gate is unchanged: Ask is still forbidden.
    assert_eq!(
        Gate::new()
            .submit_with(&who(), vec![rm("k3", "f")], &facts)
            .unwrap_err(),
        Refusal::Forbidden {
            tier: RiskTier::Delete,
            policy: Policy::Ask
        }
    );
}

#[test]
fn an_approval_does_not_lift_a_deny_policy() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    approvals
        .decide(&id, Verdict::Approve, "owner:o", 20)
        .unwrap();
    let facts = TreeFacts::new(room.nodes());
    let mut deny = approvals.gate(|_, _| Policy::Deny);
    let r = approvals.submit(&mut deny, &who(), "file_1", rm("k1", "f"), &facts, 30);
    assert!(matches!(
        r,
        Err(ApprovalRefusal::Refused(Refusal::Forbidden { .. }))
    ));
    assert_eq!(approvals.get(&id).unwrap().state, ApprovalState::Approved);
}
