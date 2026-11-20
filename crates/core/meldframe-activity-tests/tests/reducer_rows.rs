//! The reducer's rows, lanes and runs on scripted runs: two agents and a person (ADR-0021, slice 8).

// Under WASM these tests will run through wasm-bindgen-test once `just wasm-test` lists this crate (slice 10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::*;
use meldframe_activity::{Review, Status};

#[test]
fn two_agents_and_a_person_make_two_rows_newest_first() {
    let mut a = activity();
    // The person's own edits have editor keys, not batch keys.
    a.observe_op(&op(1, ME, "ed-1-n1", add("t", "Title", None)), 1_000);
    a.observe_op(&op(2, "agent:a", "ka:0", add("f", "Hero", None)), 2_000);
    a.observe_op(
        &op(3, "agent:a", "ka:1", add("r", "Card", Some("f"))),
        2_010,
    );
    a.observe_op(&op(4, "agent:a", "ka:2", set_x("t", 5.0)), 2_020);
    a.observe_op(&op(5, "agent:b", "kb:0", remove("t")), 3_000);
    let mut ha = header(1, 1, "agent:a", "ka", 3, 2_000);
    ha.first_seq = 2;
    ha.last_seq = 4;
    let mut hb = header(2, 2, "agent:b", "kb", 1, 3_000);
    hb.first_seq = 5;
    hb.last_seq = 5;
    assert_eq!(a.observe_header(ha, 2), None);
    assert_eq!(a.observe_header(hb, 2), None);
    a.tick(3_500);

    let rows = a.rows();
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].n, rows[0].actor.as_str()), (Some(2), "agent:b"));
    assert_eq!((rows[1].n, rows[1].actor.as_str()), (Some(1), "agent:a"));
    assert!(rows.iter().all(|r| r.status == Status::Applied));
    let s = rows[1].summary.as_ref().unwrap();
    assert_eq!((s.added, s.changed, s.removed), (2, 1, 0));
    assert_eq!(s.names, ["Hero", "Card", "t"]);
    assert!(rows[1].can_show);
    assert_eq!(a.show_ids("agent:a", "ka"), ["f", "r", "t"]);

    assert_eq!(a.unreviewed_count(), 2);
}

#[test]
fn a_partial_batch_says_how_many_were_written_and_the_log_wins_over_the_cache() {
    let mut a = activity();
    let mut h = header(1, 1, "agent:a", "k", 30, 1_000);
    h.written = 18;
    a.observe_header(h, 1);
    let r = &a.rows()[0];
    assert_eq!((r.status, r.written, r.commands), (Status::Partial, 18, 30));
    // Two more ops are held than the cache says: the row counts them.
    for i in 0..20 {
        a.observe_op(
            &op(100 + i, "agent:a", &format!("k:{i}"), set_x("a", i as f64)),
            1_000,
        );
    }
    let r = &a.rows()[0];
    assert_eq!((r.status, r.written, r.commands), (Status::Partial, 20, 30));
}

#[test]
fn a_header_only_row_has_no_summary_and_no_show() {
    let mut a = activity();
    a.observe_header(header(7, 1, "agent:a", "old", 4, 1_000), 1);
    let r = &a.rows()[0];
    assert_eq!((r.n, r.status, r.written), (Some(7), Status::Applied, 4));
    assert!(r.summary.is_none());
    assert!(!r.can_show);
    assert!(a.show_ids("agent:a", "old").is_empty());
}

#[test]
fn a_replay_after_a_reconnect_counts_each_op_once() {
    let mut a = activity();
    let ops: Vec<_> = (0..3u64)
        .map(|i| {
            op(
                10 + i,
                "agent:a",
                &format!("k:{i}"),
                add(&format!("n{i}"), "N", None),
            )
        })
        .collect();
    for o in &ops {
        a.observe_op(o, 1_000);
    }
    let before = a.rows();
    for o in &ops {
        a.observe_op(o, 50_000);
    }
    assert_eq!(a.rows(), before);
    assert_eq!(a.rows()[0].written, 3);
}

#[test]
fn the_highest_rev_wins_per_n_and_the_count_of_the_highest_rev_stays() {
    let mut a = activity();
    a.observe_header(header(1, 1, "agent:a", "k", 1, 1_000), 1);
    let mut accepted = header(1, 2, "agent:a", "k", 1, 1_000);
    accepted.review = Review::Accepted;
    accepted.reviewed_by = Some(ME.into());
    accepted.reviewed_at_ms = Some(5_000);
    a.observe_header(accepted, 0);
    // A frame of rev 1 that arrives late (two writers published in the other order) changes nothing.
    a.observe_header(header(1, 1, "agent:a", "k", 1, 1_000), 1);
    let r = &a.rows()[0];
    assert_eq!((r.rev, r.review), (2, Review::Accepted));
    assert_eq!(
        (r.reviewed_by.as_deref(), r.reviewed_at_ms),
        (Some(ME), Some(5_000))
    );
    assert_eq!(a.unreviewed_count(), 0);
}

#[test]
fn a_note_is_checked_again_and_a_bad_header_is_dropped() {
    let mut a = activity();
    let mut h = header(1, 1, "agent:a", "k", 1, 1_000);
    h.note = Some("  fix\u{202E}ed\u{200B} the hero  ".into());
    a.observe_header(h, 1);
    assert_eq!(
        a.rows()[0].note.as_ref().unwrap().as_str(),
        "fixed the hero"
    );
    for bad in [
        header(0, 9, "agent:a", "k0", 1, 1),
        header(2, 9, "", "k2", 1, 1),
        header(3, 9, "agent:\u{200B}a", "k3", 1, 1),
        header(4, 9, "agent:a", &"p".repeat(101), 1, 1),
    ] {
        a.observe_header(bad, 1);
    }
    assert_eq!(a.rows().len(), 1);
    // A hidden-only note is no note; a name in a summary is cleaned the same way.
    let mut h = header(5, 10, "agent:a", "k5", 1, 1);
    h.note = Some("\u{200B}\u{202E}".into());
    a.observe_header(h, 1);
    a.observe_op(
        &op(9, "agent:a", "k5:0", add("x", "Ti\u{202E}tle", None)),
        1,
    );
    let r = a.rows().into_iter().find(|r| r.n == Some(5)).unwrap();
    assert!(r.note.is_none());
    assert_eq!(r.summary.unwrap().names, ["Title"]);
}

#[test]
fn a_summary_names_at_most_three_nodes_of_at_most_forty_characters() {
    let mut a = activity();
    for i in 0..5u64 {
        a.observe_op(
            &op(
                i + 1,
                "agent:a",
                &format!("k:{i}"),
                add(&format!("n{i}"), &format!("{i}{}", "N".repeat(60)), None),
            ),
            1,
        );
    }
    let s = a.rows()[0].summary.clone().unwrap();
    assert_eq!(s.added, 5);
    assert_eq!(s.names.len(), 3);
    assert!(s.names.iter().all(|n| n.chars().count() == 40));
}
