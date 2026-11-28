//! The admission gate (ADR-0010): one decision for every op, a permit only when it is admitted.

mod gate_common;

use gate_common::{add, op, remove};
use meldframe_host::{Gate, Principal, Refusal};
use meldframe_module::{Policy, RiskTier};
use meldframe_protocol::MAX_OP_DEPS;

fn who() -> Principal {
    Principal::new("ws_1", "user:a")
}

#[test]
fn a_change_is_admitted_and_the_permit_names_its_actor_op_and_workspace() {
    let mut gate = Gate::new();
    let report = gate.submit(&who(), vec![op("k1", add("n1"))]).unwrap();
    assert_eq!(report.admitted.len(), 1);
    let a = &report.admitted[0];
    assert_eq!(a.op().key, "k1");
    assert_eq!(a.permit().actor(), "user:a");
    assert_eq!(a.permit().workspace(), "ws_1");
    assert_eq!(a.permit().key(), "k1");
}

#[test]
fn a_batch_is_admitted_whole_in_order() {
    let mut gate = Gate::new();
    let ops = vec![op("k1", add("n1")), op("k2", add("n2"))];
    let report = gate.submit(&who(), ops).unwrap();
    let keys: Vec<&str> = report
        .admitted
        .iter()
        .map(|a| a.op().key.as_str())
        .collect();
    assert_eq!(keys, ["k1", "k2"]);
}

#[test]
fn a_delete_is_forbidden_by_the_default_policy_because_it_is_ask() {
    let mut gate = Gate::new();
    let err = gate
        .submit(&who(), vec![op("k1", remove("n1"))])
        .unwrap_err();
    assert_eq!(
        err,
        Refusal::Forbidden {
            tier: RiskTier::Delete,
            policy: Policy::Ask
        }
    );
    assert_eq!(err.code(), "forbidden");
}

#[test]
fn a_policy_override_can_allow_a_delete_for_one_actor() {
    let mut gate = Gate::with_policy(|p, tier| {
        if tier == RiskTier::Delete && p.actor == "user:a" {
            Policy::Auto
        } else {
            meldframe_module::default_policy(tier)
        }
    });
    assert!(gate.submit(&who(), vec![op("k1", remove("n1"))]).is_ok());
    let other = Principal::new("ws_1", "agent:b");
    assert!(gate.submit(&other, vec![op("k2", remove("n1"))]).is_err());
}

#[test]
fn when_policy_cannot_be_read_only_reads_pass() {
    let mut gate = Gate::reads_only();
    let err = gate.submit(&who(), vec![op("k1", add("n1"))]).unwrap_err();
    assert_eq!(
        err,
        Refusal::Forbidden {
            tier: RiskTier::Change,
            policy: Policy::Deny
        }
    );
}

#[test]
fn one_forbidden_op_refuses_the_whole_batch() {
    let mut gate = Gate::with_quota(10);
    let ops = vec![op("k1", add("n1")), op("k2", remove("n1"))];
    assert_eq!(gate.submit(&who(), ops).unwrap_err().code(), "forbidden");
    assert_eq!(gate.used("ws_1"), 0, "a refused batch is not charged");
}

#[test]
fn the_quota_counts_ops_per_workspace_and_a_refused_batch_charges_nothing() {
    let mut gate = Gate::with_quota(3);
    gate.submit(&who(), vec![op("k1", add("n1")), op("k2", add("n2"))])
        .unwrap();
    let err = gate
        .submit(&who(), vec![op("k3", add("n3")), op("k4", add("n4"))])
        .unwrap_err();
    assert_eq!(err, Refusal::QuotaExceeded { limit: 3 });
    assert_eq!(err.code(), "quota_exceeded");
    assert!(err.retryable());
    assert_eq!(gate.used("ws_1"), 2);
    gate.submit(&who(), vec![op("k5", add("n5"))]).unwrap();
    let other = Principal::new("ws_2", "user:a");
    gate.submit(&other, vec![op("k1", add("n1"))]).unwrap();
}

#[test]
fn a_malformed_op_is_invalid_and_does_not_use_quota() {
    let mut gate = Gate::with_quota(10);
    let p = who();
    for ops in [
        vec![],
        vec![op("", add("n1"))],
        vec![op("k1", add("n1")), op("k1", add("n2"))],
        vec![op("k1", add("n1")).with_deps(["k1"])],
        vec![op("k1", add("n1")).with_deps([""])],
        vec![op("k1", add("n1")).with_deps(["d", "d"])],
        vec![op("k1", add("n1")).with_deps((0..=MAX_OP_DEPS).map(|i| format!("d{i}")))],
    ] {
        let err = gate.submit(&p, ops).unwrap_err();
        assert_eq!(err.code(), "invalid", "{err}");
        assert!(!err.retryable());
    }
    assert_eq!(gate.used("ws_1"), 0);
}

#[test]
fn exactly_the_most_dependencies_are_allowed() {
    let mut gate = Gate::new();
    let deps = (0..MAX_OP_DEPS).map(|i| format!("d{i}"));
    assert!(
        gate.submit(&who(), vec![op("k1", add("n1")).with_deps(deps)])
            .is_ok()
    );
}

#[test]
fn invalid_is_decided_before_policy() {
    let mut gate = Gate::new();
    let ops = vec![op("", remove("n1"))];
    assert_eq!(gate.submit(&who(), ops).unwrap_err().code(), "invalid");
}

#[test]
fn a_quota_and_a_policy_override_can_be_combined() {
    let mut gate = Gate::with_policy(|_, _| Policy::Auto).quota(1);
    let p = who();
    assert!(gate.submit(&p, vec![op("k1", remove("n1"))]).is_ok());
    let err = gate.submit(&p, vec![op("k2", add("n2"))]).unwrap_err();
    assert_eq!(err.code(), "quota_exceeded");
}

#[test]
fn the_forbidden_text_names_the_policy_in_words_that_do_not_depend_on_debug() {
    let mut gate = Gate::new();
    let err = gate
        .submit(&who(), vec![op("k1", remove("n1"))])
        .unwrap_err();
    assert!(err.to_string().contains("policy ask"), "{err}");
}
