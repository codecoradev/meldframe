//! The call log seam: a bounded, in-order log of calls with a decision and an outcome.

use meldframe_calls::{CallEntry, CallLog, Decision, MemoryCallLog};
use meldframe_module::RiskTier;

fn entry(at: u64) -> CallEntry {
    CallEntry {
        at,
        workspace: "ws".into(),
        actor: "agent:a".into(),
        tool: "paint".into(),
        file: "f".into(),
        risk: Some(RiskTier::Change),
        decision: Some(Decision::Allow),
        outcome: "ok".into(),
    }
}

#[test]
fn keeps_entries_in_order_and_returns_the_newest_within_a_limit() {
    let mut log = MemoryCallLog::with_capacity(10);
    for at in 1..=5 {
        log.record(entry(at));
    }
    let all: Vec<u64> = log.recent(10).iter().map(|e| e.at).collect();
    assert_eq!(all, [1, 2, 3, 4, 5]);
    let two: Vec<u64> = log.recent(2).iter().map(|e| e.at).collect();
    assert_eq!(two, [4, 5]);
}

#[test]
fn drops_the_oldest_when_full() {
    let mut log = MemoryCallLog::with_capacity(3);
    for at in 1..=5 {
        log.record(entry(at));
    }
    let kept: Vec<u64> = log.recent(10).iter().map(|e| e.at).collect();
    assert_eq!(kept, [3, 4, 5]);
}

#[test]
fn a_decision_has_a_stable_name() {
    assert_eq!(
        [
            Decision::Allow.as_str(),
            Decision::Ask.as_str(),
            Decision::Deny.as_str()
        ],
        ["allow", "ask", "deny"]
    );
}
