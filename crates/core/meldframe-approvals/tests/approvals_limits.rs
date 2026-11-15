//! E2.4: what an approval keeps and what it does not lift: quota, workspace, the whole op, the tier, expiry
//! limits, panics and sibling gates.

use common::*;
use meldframe_approvals::{
    ApprovalGate, ApprovalRefusal, ApprovalState, Approvals, DecideError, Verdict,
};
use meldframe_host::{Gate, MemoryStore, NodeFacts, Principal, Refusal, Room};
use meldframe_module::{Policy, RiskTier, default_policy};
use meldframe_rooms::TreeFacts;

mod common;

#[test]
fn an_approved_op_still_meets_the_workspace_quota() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let mut gate = approvals.gate(|_, tier| default_policy(tier)).quota(1);
    let id = match submit_as(&mut approvals, &mut gate, &who(), &room, rm("k1", "f"), 10) {
        Err(ApprovalRefusal::Required { id }) => id,
        other => panic!("{other:?}"),
    };
    approve(&mut approvals, &id);
    // The workspace uses its one op elsewhere; the approved delete is then over the quota.
    gate.submit(&who(), vec![add("k-x", "x", "rect", Some("f"))])
        .unwrap();
    let r = submit_as(&mut approvals, &mut gate, &who(), &room, rm("k1", "f"), 30);
    assert_eq!(
        r.unwrap_err(),
        ApprovalRefusal::Refused(Refusal::QuotaExceeded { limit: 1 })
    );
    // A refused admission does not spend the approval.
    assert_eq!(approvals.get(&id).unwrap().state, ApprovalState::Approved);
}

#[test]
fn an_admitted_approved_op_is_counted_in_the_gate() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let mut gate = approvals.gate(|_, tier| default_policy(tier)).quota(5);
    let id = match submit_as(&mut approvals, &mut gate, &who(), &room, rm("k1", "f"), 10) {
        Err(ApprovalRefusal::Required { id }) => id,
        other => panic!("{other:?}"),
    };
    assert_eq!(gate.used("ws_1"), 0, "a refused call is not counted");
    approve(&mut approvals, &id);
    submit_as(&mut approvals, &mut gate, &who(), &room, rm("k1", "f"), 30).unwrap();
    assert_eq!(gate.used("ws_1"), 1);
}

#[test]
fn an_approval_does_not_cover_another_workspace() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    approve(&mut approvals, &id);
    let other = Principal::new("ws_2", "agent:a");
    let mut g = dflt(&approvals);
    let r = submit_as(&mut approvals, &mut g, &other, &room, rm("k1", "f"), 30);
    assert!(matches!(r, Err(ApprovalRefusal::Required { .. })));
    assert_eq!(approvals.get(&id).unwrap().state, ApprovalState::Approved);
    assert!(approvals.find("ws_2", "file_1", "agent:a", "k1").is_some());
    assert_eq!(
        approvals
            .find("ws_1", "file_1", "agent:a", "k1")
            .unwrap()
            .id,
        id
    );
}

#[test]
fn an_approval_covers_the_whole_op() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    approve(&mut approvals, &id);
    for other in [
        rm("k1", "f").with_if_seq(1),
        rm("k1", "f").with_deps(["k0"]),
    ] {
        let r = submit(&mut approvals, &room, other, 30);
        assert!(matches!(r, Err(ApprovalRefusal::Required { id: new }) if new != id));
    }
    assert_eq!(approvals.get(&id).unwrap().state, ApprovalState::Approved);
    submit(&mut approvals, &room, rm("k1", "f"), 31).unwrap();
}

#[test]
fn the_permit_carries_the_tier_of_the_real_gate() {
    // A policy that asks for a Change: a leaf delete is a small delete, a Change by the facts.
    let mut room = room();
    let leaf = add("k-r", "r", "rect", Some("f"));
    for adm in Gate::new().submit(&who(), vec![leaf]).unwrap().admitted {
        room.accept(adm).unwrap();
    }
    let mut approvals = Approvals::new(TTL);
    let ask_change = |_: &Principal, t: RiskTier| match t {
        RiskTier::Change => Policy::Ask,
        t => default_policy(t),
    };
    let mut gate = approvals.gate(ask_change);
    let r = submit_as(&mut approvals, &mut gate, &who(), &room, rm("k1", "r"), 10);
    let Err(ApprovalRefusal::Required { id }) = r else {
        panic!("{r:?}")
    };
    approve(&mut approvals, &id);
    let report = submit_as(&mut approvals, &mut gate, &who(), &room, rm("k1", "r"), 30).unwrap();
    assert_eq!(report.admitted[0].permit().tier(), RiskTier::Change);
}

#[test]
fn the_expiry_boundary_is_exact() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    approvals
        .decide(&id, Verdict::Approve, "owner:o", 10 + TTL - 1)
        .unwrap();
    assert!(submit(&mut approvals, &room, rm("k1", "f"), 10 + TTL - 1).is_ok());
    let id = ask(&mut approvals, &room, "k2", "f", 10);
    approvals
        .decide(&id, Verdict::Approve, "owner:o", 20)
        .unwrap();
    let r = submit(&mut approvals, &room, rm("k2", "f"), 10 + TTL);
    assert_eq!(r.unwrap_err(), ApprovalRefusal::Expired { id });
}

#[test]
fn a_ttl_of_zero_expires_at_once() {
    let (room, mut approvals) = (room(), Approvals::new(0));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    assert_eq!(approvals.get(&id).unwrap().expires_at, 10);
    let r = approvals.decide(&id, Verdict::Approve, "owner:o", 10);
    assert_eq!(r.unwrap_err(), DecideError::Expired { id });
}

#[test]
fn a_huge_ttl_saturates_instead_of_wrapping() {
    let (room, mut approvals) = (room(), Approvals::new(u64::MAX));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    assert_eq!(approvals.get(&id).unwrap().expires_at, u64::MAX);
    approve(&mut approvals, &id);
    assert!(submit(&mut approvals, &room, rm("k1", "f"), u64::MAX - 1).is_ok());
}

/// Facts that behave as `()` once and panic on the second read.
struct PanicsSecond(std::cell::Cell<u32>);

impl NodeFacts for PanicsSecond {
    fn delete_is_small(&self, _: &str) -> bool {
        self.0.set(self.0.get() + 1);
        assert!(self.0.get() < 2, "facts failed");
        false
    }
}

/// An unapproved Ask op of another key and node must still be refused on `gate`, and on a sibling gate.
fn assert_unapproved_ask_is_refused(
    approvals: &mut Approvals,
    gate: &mut ApprovalGate,
    room: &Room<MemoryStore>,
) {
    let facts = TreeFacts::new(room.nodes());
    let r = approvals.submit(gate, &who(), "file_1", rm("other", "g"), &facts, 40);
    assert!(matches!(r, Err(ApprovalRefusal::Required { .. })), "{r:?}");
    let forbidden = Refusal::Forbidden {
        tier: RiskTier::Delete,
        policy: Policy::Ask,
    };
    assert_eq!(
        gate.submit_with(&who(), vec![rm("plain", "g")], &facts)
            .unwrap_err(),
        forbidden
    );
    let mut sibling = dflt(approvals);
    assert_eq!(
        sibling
            .submit_with(&who(), vec![rm("plain", "g")], &facts)
            .unwrap_err(),
        forbidden
    );
}

#[test]
fn a_panic_while_admitting_does_not_leave_ask_lifted() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let mut gate = dflt(&approvals);
    let id = match submit_as(&mut approvals, &mut gate, &who(), &room, rm("k1", "f"), 10) {
        Err(ApprovalRefusal::Required { id }) => id,
        other => panic!("{other:?}"),
    };
    approve(&mut approvals, &id);
    let facts = PanicsSecond(std::cell::Cell::new(0));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = approvals.submit(&mut gate, &who(), "file_1", rm("k1", "f"), &facts, 30);
    }));
    assert!(caught.is_err());
    assert_unapproved_ask_is_refused(&mut approvals, &mut gate, &room);
}

#[test]
fn ask_stays_closed_after_a_failed_and_after_a_successful_approved_admission() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let mut gate = approvals.gate(|_, tier| default_policy(tier)).quota(1);
    let id = match submit_as(&mut approvals, &mut gate, &who(), &room, rm("k1", "f"), 10) {
        Err(ApprovalRefusal::Required { id }) => id,
        other => panic!("{other:?}"),
    };
    approve(&mut approvals, &id);
    gate.submit(&who(), vec![add("k-x", "x", "rect", Some("f"))])
        .unwrap();
    let failed = submit_as(&mut approvals, &mut gate, &who(), &room, rm("k1", "f"), 30);
    assert!(matches!(
        failed,
        Err(ApprovalRefusal::Refused(Refusal::QuotaExceeded { .. }))
    ));
    assert_unapproved_ask_is_refused(&mut approvals, &mut gate, &room);
    let mut gate = approvals.gate(|_, tier| default_policy(tier));
    submit_as(&mut approvals, &mut gate, &who(), &room, rm("k1", "f"), 31).unwrap();
    assert_unapproved_ask_is_refused(&mut approvals, &mut gate, &room);
}

#[test]
fn an_approval_is_consumed_when_the_op_is_admitted_without_it() {
    let (room, mut approvals) = (room(), Approvals::new(TTL));
    let id = ask(&mut approvals, &room, "k1", "f", 10);
    approve(&mut approvals, &id);
    // The policy changed to Auto: the op is admitted, and the approval is spent with it.
    let mut auto = approvals.gate(|_, _| Policy::Auto);
    submit_as(&mut approvals, &mut auto, &who(), &room, rm("k1", "f"), 30).unwrap();
    assert_eq!(approvals.get(&id).unwrap().state, ApprovalState::Consumed);
    let again = ask(&mut approvals, &room, "k1", "f", 31);
    assert_ne!(again, id);
}
