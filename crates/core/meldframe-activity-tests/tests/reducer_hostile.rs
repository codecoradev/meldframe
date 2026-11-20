//! What a socket or a restart can do to the reducer: absurd revs, a log that starts again, a header that names a
//! second `n`, and the summary of a batch that adds, changes and removes the same nodes (ADR-0021).

// Under WASM these tests will run through wasm-bindgen-test once `just wasm-test` lists this crate (slice 10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::*;
use meldframe_activity::{MAX_REV, MAX_REV_JUMP};

fn adds(n: u64, prefix: &str) -> Vec<meldframe_revert::LoggedOp> {
    (0..n)
        .map(|i| {
            op(
                i + 1,
                "agent:a",
                &format!("{prefix}:{i}"),
                add(&format!("n{i}"), "N", None),
            )
        })
        .collect()
}

#[test]
fn a_rev_past_the_ceiling_is_dropped_and_poisons_nothing() {
    let mut a = activity();
    for rev in [u64::MAX, MAX_REV + 1] {
        assert_eq!(
            a.observe_header(header(1, rev, "agent:a", "k", 1, 1), 9),
            None
        );
    }
    a.observe_page([header(1, u64::MAX, "agent:a", "k", 1, 1)], 9);
    assert!(a.rows().is_empty());
    assert_eq!((a.highest_rev(), a.unreviewed_count()), (0, 0));
    // The next honest header is no gap and sets the count.
    assert_eq!(
        a.observe_header(header(2, 1, "agent:a", "k2", 1, 2), 3),
        None
    );
    assert_eq!(a.unreviewed_count(), 3);
    // The ceiling itself is a rev (a page is the truth: no jump bound).
    a.observe_page([header(3, MAX_REV, "agent:a", "k3", 1, 3)], 4);
    assert_eq!(a.highest_rev(), MAX_REV);
    assert_eq!(
        a.observe_header(header(4, MAX_REV, "agent:a", "k4", 1, 4), 4),
        None
    );
}

#[test]
fn a_frame_that_jumps_past_the_bound_is_not_taken_and_asks_for_a_fetch() {
    let mut a = activity();
    a.observe_header(header(1, 1, "agent:a", "k1", 1, 1), 1);
    let far = header(2, 1 + MAX_REV_JUMP + 1, "agent:a", "k2", 1, 2);
    assert!(a.observe_header(far, 7).is_some());
    assert_eq!(
        (a.rows().len(), a.highest_rev(), a.unreviewed_count()),
        (1, 1, 1)
    );
    // Up to the bound it is a gap, taken in (and asked for, once the 5 seconds between fetches have passed).
    a.clock(10_000);
    let near = header(3, 1 + MAX_REV_JUMP, "agent:a", "k3", 1, 3);
    assert!(a.observe_header(near, 2).is_some());
    assert_eq!((a.rows().len(), a.unreviewed_count()), (2, 2));
}

#[test]
fn reset_log_lets_a_log_that_starts_again_count_and_forgets_the_old_ops() {
    let mut a = activity();
    for o in adds(50, "old") {
        a.observe_op(&o, 1_000);
    }
    a.observe_header(header(1, 1, "agent:a", "old", 50, 1_000), 1);
    // The server restarted without a database: seq starts at 1 again.
    a.reset_log(0);
    for o in adds(3, "new") {
        a.observe_op(&o, 2_000);
    }
    let rows = a.rows();
    let new = rows.iter().find(|r| r.key_prefix == "new").unwrap();
    assert_eq!(new.written, 3);
    // The header kept its row, with no summary and no Show, as after a reload.
    let old = rows.iter().find(|r| r.key_prefix == "old").unwrap();
    assert_eq!(
        (old.n, old.written, old.summary.is_none(), old.can_show),
        (Some(1), 50, true, false)
    );
}

#[test]
fn a_resync_below_the_highest_seq_counts_the_replayed_ops_once() {
    let mut a = activity();
    let ops = adds(10, "k");
    for o in &ops {
        a.observe_op(o, 1_000);
    }
    // The snapshot is at seq 5: the stream replays 6 to 10; a batch with no header is rebuilt from them.
    a.reset_log(5);
    assert!(a.rows().is_empty());
    for o in &ops[2..] {
        a.observe_op(o, 2_000);
    }
    let r = &a.rows()[0];
    assert_eq!((r.written, r.first_seq, r.last_seq), (5, 6, 10));
}

#[test]
fn a_header_refused_for_a_second_n_changes_nothing() {
    let mut a = activity();
    a.observe_header(header(1, 5, "agent:a", "k1", 1, 1), 2);
    a.observe_header(header(2, 1, "agent:b", "k2", 1, 2), 2);
    // n 2 now names the batch that holds n 1 at a higher rev: refused, and the batch it was on stays.
    a.observe_header(header(2, 3, "agent:a", "k1", 1, 3), 2);
    let ns: Vec<_> = a.rows().iter().map(|r| r.n).collect();
    assert_eq!(ns.len(), 2);
    assert!(ns.contains(&Some(1)) && ns.contains(&Some(2)));
    assert_eq!((a.lowest_n(), a.highest_n()), (Some(1), Some(2)));
}

#[test]
fn a_node_added_and_removed_in_one_batch_is_neither_shown_nor_counted() {
    let mut a = activity();
    a.observe_op(&op(1, "agent:a", "k:0", add("x", "X", None)), 1);
    a.observe_op(&op(2, "agent:a", "k:1", remove("x")), 1);
    let s = a.rows()[0].summary.clone().unwrap();
    assert_eq!((s.added, s.changed, s.removed), (0, 0, 0));
    assert!(a.show_ids("agent:a", "k").is_empty());
    assert!(!a.rows()[0].can_show);
}

#[test]
fn a_node_changed_and_then_removed_is_a_removal_and_is_not_shown() {
    let mut a = activity();
    a.observe_op(&op(1, "agent:a", "k:0", set_x("a", 3.0)), 1);
    a.observe_op(&op(2, "agent:a", "k:1", set_x("b", 3.0)), 1);
    a.observe_op(&op(3, "agent:a", "k:2", remove("a")), 1);
    let s = a.rows()[0].summary.clone().unwrap();
    assert_eq!((s.added, s.changed, s.removed), (0, 1, 1));
    assert_eq!(a.show_ids("agent:a", "k"), ["b"]);
}

#[test]
fn counts_past_the_tracked_ids_keep_counting() {
    let mut a = activity();
    for o in adds(5_000, "k") {
        a.observe_op(&o, 1);
    }
    let s = a.rows()[0].summary.clone().unwrap();
    assert_eq!(s.added, 5_000);
    assert!(a.show_ids("agent:a", "k").len() <= meldframe_activity::MAX_SHOWN);

    let mut b = activity();
    for i in 0..5_000u64 {
        b.observe_op(
            &op(
                i + 1,
                "agent:a",
                &format!("k:{i}"),
                set_x(&format!("n{i}"), 1.0),
            ),
            1,
        );
    }
    assert_eq!(b.rows()[0].summary.clone().unwrap().changed, 5_000);
}

#[test]
fn a_node_name_gets_the_combining_run_cap_of_a_note() {
    let mut a = activity();
    let name = format!("e{}x", "\u{0301}".repeat(9));
    a.observe_op(&op(1, "agent:a", "k:0", add("x", &name, None)), 1);
    let names = a.rows()[0].summary.clone().unwrap().names;
    assert_eq!(names, [format!("e{}x", "\u{0301}".repeat(4))]);
}

#[test]
fn the_jump_fetch_is_asked_for_once_in_the_fetch_window() {
    let mut a = activity();
    a.observe_header(header(1, 1, "agent:a", "k1", 1, 1), 1);
    let far = |a: &mut meldframe_activity::Activity| {
        a.observe_header(header(2, 5_000_000, "agent:a", "k2", 1, 2), 9)
    };
    a.tick(10_000);
    assert!(far(&mut a).is_some());
    a.tick(14_999);
    assert!(far(&mut a).is_none());
    a.tick(15_000);
    assert!(far(&mut a).is_some());
    // None of them was taken in.
    assert_eq!(
        (a.rows().len(), a.highest_rev(), a.unreviewed_count()),
        (1, 1, 1)
    );
}

#[test]
fn the_first_frame_of_a_fresh_reducer_cannot_freeze_the_count_either() {
    let mut a = activity();
    assert!(
        a.observe_header(header(1, MAX_REV, "agent:a", "k", 1, 1), 9)
            .is_some()
    );
    assert_eq!(
        (a.rows().len(), a.highest_rev(), a.unreviewed_count()),
        (0, 0, 0)
    );
    assert_eq!(
        a.observe_header(header(2, 1, "agent:a", "k2", 1, 2), 3),
        None
    );
    assert_eq!(a.unreviewed_count(), 3);
}

#[test]
fn a_large_but_true_jump_heals_through_a_page() {
    let mut a = activity();
    let rev = 2_000_000;
    assert!(
        a.observe_header(header(7, rev, "agent:a", "k", 1, 1), 4)
            .is_some()
    );
    assert!(a.rows().is_empty());
    // The fetch it asked for answers: a page has only the ceiling to keep.
    a.observe_page([header(7, rev, "agent:a", "k", 1, 1)], 4);
    assert_eq!(
        (a.rows().len(), a.highest_rev(), a.unreviewed_count()),
        (1, rev, 4)
    );
    assert_eq!(
        a.observe_header(header(8, rev + 1, "agent:a", "k8", 1, 2), 5),
        None
    );
    assert_eq!(a.unreviewed_count(), 5);
}

#[test]
fn reset_log_keeps_header_numbering_and_reset_all_starts_from_nothing() {
    let mut a = activity();
    a.observe_header(header(1, 5, "agent:a", "k", 1, 1), 3);
    // A restart that numbers n and rev from 1 again: reset_log keeps the old numbering, so the new header is stale.
    a.reset_log(0);
    a.observe_header(header(1, 1, "agent:b", "other", 1, 2), 1);
    assert_eq!(
        (a.rows()[0].actor.as_str(), a.unreviewed_count()),
        ("agent:a", 3)
    );
    a.reset_all(0);
    assert!(a.rows().is_empty());
    assert_eq!((a.highest_rev(), a.unreviewed_count(), a.me()), (0, 0, ME));
    a.observe_header(header(1, 1, "agent:b", "other", 1, 2), 1);
    assert_eq!(
        (a.rows()[0].actor.as_str(), a.unreviewed_count()),
        ("agent:b", 1)
    );
    // Ops of the new log count from seq 1.
    a.observe_op(&op(1, "agent:b", "other:0", set_x("a", 1.0)), 5);
    assert_eq!(a.rows()[0].written, 1);
}

#[test]
fn reset_log_forgets_a_persons_runs_too_so_a_replay_does_not_count_twice() {
    let mut a = activity();
    let ops: Vec<_> = (1..=3)
        .map(|i| op(i, ME, &format!("ed-{i}"), set_x("a", 1.0)))
        .collect();
    for o in &ops {
        a.observe_op(o, 1_000);
    }
    a.reset_log(0);
    for o in &ops {
        a.observe_op(o, 1_000);
    }
    assert_eq!(a.lanes()[0].ops, 3);
}
