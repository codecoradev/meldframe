//! E2.3: per-workspace quotas at the gate: ops per minute, batch size and storage, each with its own stable code.
//! A refused batch charges nothing, and another workspace is never affected.

mod common;

use common::{add, op};
use meldframe_host::{Gate, Principal, Refusal};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

fn who(ws: &str) -> Principal {
    Principal::new(ws, "user:a")
}

/// A clock the test moves by hand, in milliseconds.
fn clock() -> (Arc<AtomicU64>, impl Fn() -> u64 + Send + Sync + 'static) {
    let t = Arc::new(AtomicU64::new(0));
    let t2 = t.clone();
    (t, move || t2.load(Ordering::SeqCst))
}

fn ops(prefix: &str, n: usize) -> Vec<meldframe_host::Op> {
    (0..n)
        .map(|i| op(&format!("{prefix}{i}"), add(&format!("{prefix}n{i}"))))
        .collect()
}

#[test]
fn ops_per_minute_refuse_the_batch_that_goes_over_and_charge_nothing() {
    let (t, now) = clock();
    let mut gate = Gate::new().ops_per_minute(5, now);
    gate.submit(&who("a"), ops("x", 3)).unwrap();
    t.store(20_000, Ordering::SeqCst);
    let err = gate.submit(&who("a"), ops("y", 3)).unwrap_err();
    assert_eq!(
        err,
        Refusal::RateExceeded {
            limit: 5,
            retry_after_ms: 40_000
        }
    );
    assert_eq!(err.code(), "quota_ops_per_minute");
    assert!(err.retryable());
    // The refused batch was not counted: two more still fit in this minute.
    gate.submit(&who("a"), ops("z", 2)).unwrap();
}

#[test]
fn the_minute_window_resets_and_workspaces_are_counted_apart() {
    let (t, now) = clock();
    let mut gate = Gate::new().ops_per_minute(2, now);
    gate.submit(&who("a"), ops("x", 2)).unwrap();
    gate.submit(&who("b"), ops("x", 2)).unwrap();
    assert!(gate.submit(&who("a"), ops("y", 1)).is_err());
    t.store(60_000, Ordering::SeqCst);
    gate.submit(&who("a"), ops("y", 2)).unwrap();
}

#[test]
fn a_batch_over_the_batch_limit_is_batch_too_large_and_not_invalid() {
    let mut gate = Gate::new().batch_limit(3);
    let err = gate.submit(&who("a"), ops("x", 4)).unwrap_err();
    assert_eq!(err, Refusal::BatchTooLarge { limit: 3 });
    assert_eq!(err.code(), "batch_too_large");
    assert!(!err.retryable());
    gate.submit(&who("a"), ops("y", 3)).unwrap();
}

#[test]
fn storage_counts_the_bytes_of_admitted_ops_per_workspace() {
    let mut gate = Gate::new().storage_limit(600);
    let mut used = 0;
    let err = loop {
        match gate.submit(&who("a"), ops(&format!("k{used}_"), 1)) {
            Ok(_) => used += 1,
            Err(e) => break e,
        }
        assert!(used < 100, "the limit never hit");
    };
    assert_eq!(err, Refusal::StorageExceeded { limit: 600 });
    assert_eq!(err.code(), "quota_storage");
    assert!(!err.retryable());
    assert!(used >= 1);
    assert!(gate.storage_used("a") <= 600);
    // Another workspace has its own room.
    gate.submit(&who("b"), ops("k", 1)).unwrap();
}

#[test]
fn a_refused_batch_charges_neither_rate_nor_storage() {
    let (_t, now) = clock();
    let mut gate = Gate::new().ops_per_minute(1, now).storage_limit(1_000_000);
    gate.submit(&who("a"), ops("x", 1)).unwrap();
    let before = gate.storage_used("a");
    assert!(gate.submit(&who("a"), ops("y", 1)).is_err());
    assert_eq!(gate.storage_used("a"), before);
}

#[test]
fn the_window_edge_is_exact_and_the_limit_itself_is_allowed() {
    let (t, now) = clock();
    let mut gate = Gate::new().ops_per_minute(2, now);
    t.store(59_999, Ordering::SeqCst);
    gate.submit(&who("a"), ops("x", 2)).unwrap();
    let err = gate.submit(&who("a"), ops("y", 1)).unwrap_err();
    assert_eq!(
        err,
        Refusal::RateExceeded {
            limit: 2,
            retry_after_ms: 1
        }
    );
    t.store(60_000, Ordering::SeqCst);
    gate.submit(&who("a"), ops("z", 2)).unwrap();
}

#[test]
fn a_clock_that_steps_back_does_not_reset_the_count() {
    let (t, now) = clock();
    let mut gate = Gate::new().ops_per_minute(1, now);
    t.store(120_000, Ordering::SeqCst);
    gate.submit(&who("a"), ops("x", 1)).unwrap();
    t.store(10_000, Ordering::SeqCst);
    assert!(gate.submit(&who("a"), ops("y", 1)).is_err());
}

#[test]
fn the_first_sight_of_a_workspace_starts_from_what_it_already_wrote() {
    let seen = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let s2 = seen.clone();
    let mut gate = Gate::new()
        .quota(5)
        .storage_limit(100_000)
        .usage(move |ws| {
            s2.fetch_add(1, Ordering::SeqCst);
            Some(if ws == "full" { (5, 99_990) } else { (0, 0) })
        });
    // `full` has used its ops and nearly all its bytes before this gate saw it (a restart).
    assert_eq!(
        gate.submit(&who("full"), ops("x", 1)).unwrap_err().code(),
        "quota_exceeded"
    );
    assert_eq!(gate.used("full"), 5);
    assert_eq!(gate.storage_used("full"), 99_990);
    gate.submit(&who("fresh"), ops("x", 1)).unwrap();
    gate.submit(&who("fresh"), ops("y", 1)).unwrap();
    // Asked once per workspace, not once per call.
    assert_eq!(seen.load(Ordering::SeqCst), 2);
}

#[test]
fn a_malformed_call_does_not_ask_for_usage() {
    let asked = std::sync::Arc::new(AtomicU64::new(0));
    let a2 = asked.clone();
    let mut gate = Gate::new().usage(move |_| {
        a2.fetch_add(1, Ordering::SeqCst);
        Some((0, 0))
    });
    assert!(gate.submit(&who("a"), vec![]).is_err());
    assert_eq!(asked.load(Ordering::SeqCst), 0);
}

#[test]
fn a_usage_lookup_that_fails_is_asked_again_on_the_next_call() {
    let calls = std::sync::Arc::new(AtomicU64::new(0));
    let c2 = calls.clone();
    let mut gate = Gate::new().quota(3).usage(move |_| {
        // The store is down on the first call and says 3 ops are used on the second.
        (c2.fetch_add(1, Ordering::SeqCst) > 0).then_some((3, 0))
    });
    gate.submit(&who("a"), ops("x", 1)).unwrap();
    assert_eq!(
        gate.submit(&who("a"), ops("y", 1)).unwrap_err().code(),
        "quota_exceeded"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn a_batch_larger_than_the_rate_itself_is_batch_too_large_not_a_wait() {
    // No minute can ever hold seven ops when the rate is five, so waiting does not help: the refusal must not
    // be retryable and must name the limit to split by.
    let (_t, now) = clock();
    let mut gate = Gate::new().ops_per_minute(5, now);
    let err = gate.submit(&who("a"), ops("x", 7)).unwrap_err();
    assert_eq!(err, Refusal::BatchTooLarge { limit: 5 });
    assert!(!err.retryable());
    gate.submit(&who("a"), ops("y", 5)).unwrap();
}

#[test]
fn a_batch_larger_than_the_rate_is_refused_before_the_policy_and_the_total_quota() {
    let (_t, now) = clock();
    // The total quota is spent and the policy denies, yet the size is what the caller must fix first.
    let mut gate = Gate::with_policy(|_, _| meldframe_module::Policy::Deny)
        .ops_per_minute(2, now)
        .quota(0);
    let err = gate.submit(&who("a"), ops("x", 3)).unwrap_err();
    assert_eq!(err, Refusal::BatchTooLarge { limit: 2 });
}

#[test]
fn a_rate_of_zero_refuses_every_batch_as_a_wait_not_as_too_large() {
    let (_t, now) = clock();
    let mut gate = Gate::new().ops_per_minute(0, now);
    let err = gate.submit(&who("a"), ops("x", 1)).unwrap_err();
    assert_eq!(err.code(), "quota_ops_per_minute");
}

#[test]
fn the_first_sight_of_a_workspace_starts_its_minute_from_what_it_wrote_in_it() {
    let (t, now) = clock();
    t.store(125_000, Ordering::SeqCst);
    let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let a2 = asked.clone();
    let mut gate = Gate::new()
        .ops_per_minute(5, now)
        .usage(|_| Some((0, 0)))
        .recent(move |ws, since_ms| {
            a2.lock().unwrap().push((ws.to_string(), since_ms));
            Some(4)
        });
    // Four ops were written in this minute before the gate existed (a restart): two more do not fit.
    assert_eq!(
        gate.submit(&who("a"), ops("x", 2)).unwrap_err().code(),
        "quota_ops_per_minute"
    );
    gate.submit(&who("a"), ops("y", 1)).unwrap();
    // Asked once, from the start of the current minute.
    assert_eq!(*asked.lock().unwrap(), [("a".to_string(), 120_000)]);
    // The next minute starts empty as usual.
    t.store(180_000, Ordering::SeqCst);
    gate.submit(&who("a"), ops("z", 5)).unwrap();
}

#[test]
fn a_recent_lookup_that_fails_is_asked_again() {
    let (_t, now) = clock();
    let calls = std::sync::Arc::new(AtomicU64::new(0));
    let c2 = calls.clone();
    let mut gate = Gate::new()
        .ops_per_minute(3, now)
        .usage(|_| Some((0, 0)))
        .recent(move |_, _| (c2.fetch_add(1, Ordering::SeqCst) > 0).then_some(3));
    gate.submit(&who("a"), ops("x", 1)).unwrap();
    // The store answers on the second call: three ops already in this minute.
    assert_eq!(
        gate.submit(&who("a"), ops("y", 1)).unwrap_err().code(),
        "quota_ops_per_minute"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn a_failed_minute_lookup_keeps_the_totals_it_already_has() {
    let (_t, now) = clock();
    let mut gate = Gate::new()
        .quota(5)
        .ops_per_minute(100, now)
        .usage(|_| Some((5, 0)))
        .recent(|_, _| None);
    // The minute is unknown, but the totals were read: the workspace is still full.
    assert_eq!(
        gate.submit(&who("a"), ops("x", 1)).unwrap_err().code(),
        "quota_exceeded"
    );
}

#[test]
fn a_late_minute_lookup_keeps_a_newer_window_the_gate_counted() {
    let (t, now) = clock();
    t.store(185_000, Ordering::SeqCst);
    let calls = std::sync::Arc::new(AtomicU64::new(0));
    let c2 = calls.clone();
    let mut gate = Gate::new()
        .ops_per_minute(3, now)
        .recent(move |_, _| (c2.fetch_add(1, Ordering::SeqCst) > 0).then_some(0));
    // The store is down: the gate counts three ops in the minute that starts at 180 000 itself.
    gate.submit(&who("a"), ops("x", 3)).unwrap();
    // The clock steps back a minute and the store answers now: the newer, full window stays.
    t.store(125_000, Ordering::SeqCst);
    assert_eq!(
        gate.submit(&who("a"), ops("y", 1)).unwrap_err().code(),
        "quota_ops_per_minute"
    );
}
