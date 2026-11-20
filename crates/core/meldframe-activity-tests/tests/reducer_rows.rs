//! The reducer's rows, lanes and runs on scripted runs: two agents and a person (ADR-0021, slice 8).

// Under WASM these tests will run through wasm-bindgen-test once `just wasm-test` lists this crate (slice 10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::*;
use meldframe_activity::{LaneKind, Review, Status};

const MIN: u64 = 60_000;

#[test]
fn two_agents_and_a_person_make_three_lanes_and_two_rows_newest_first() {
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

    let lanes = a.lanes();
    let who: Vec<_> = lanes.iter().map(|l| (l.actor.as_str(), l.kind)).collect();
    assert_eq!(
        who,
        [
            ("agent:b", LaneKind::Agent),
            ("agent:a", LaneKind::Agent),
            (ME, LaneKind::Person)
        ]
    );
    assert_eq!(lanes[1].ops, 3);
    assert_eq!(lanes[1].batches, ["ka"]);
    assert_eq!(lanes[2].ops, 1);
    assert!(lanes[2].batches.is_empty());
    assert_eq!(a.unreviewed_count(), 2);
}

#[test]
fn an_actor_that_is_not_agent_prefixed_is_a_person() {
    let mut a = activity();
    a.observe_op(&op(1, "bot:x", "k:0", add("x", "X", None)), 0);
    assert_eq!(a.lanes()[0].kind, LaneKind::Person);
}

#[test]
fn runs_split_where_the_gap_is_over_five_minutes_and_are_judged_from_at_ms() {
    let mut a = activity();
    // Person: 0 and 5 minutes exactly apart stay one run; 5 minutes and 1 ms apart split.
    for (i, t) in [0, 5 * MIN, 10 * MIN + 1].into_iter().enumerate() {
        a.observe_op(
            &op(i as u64 + 1, ME, &format!("ed-{i}"), set_x("a", 1.0)),
            t,
        );
    }
    let runs = &a.lanes()[0].runs;
    assert_eq!(runs.len(), 2);
    assert_eq!((runs[0].start_ms, runs[0].ops), (10 * MIN + 1, 1));
    assert_eq!(
        (runs[1].start_ms, runs[1].end_ms, runs[1].ops),
        (0, 5 * MIN, 2)
    );

    // An agent whose ops all arrived now (a reconnect) keeps the server's times from its headers.
    let mut b = activity();
    for (i, at) in [1_000, 2_000, 20 * MIN].into_iter().enumerate() {
        let n = i as u32 + 1;
        let p = format!("k{n}");
        b.observe_op(
            &op(u64::from(n), "agent:a", &format!("{p}:0"), set_x("a", 1.0)),
            99 * MIN,
        );
        b.observe_header(header(n, u64::from(n), "agent:a", &p, 1, at), 3);
    }
    let runs = &b.lanes()[0].runs;
    assert_eq!(runs.len(), 2);
    assert_eq!(
        (runs[1].start_ms, runs[1].end_ms, runs[1].ops),
        (1_000, 2_000, 2)
    );
}

#[test]
fn a_lane_is_working_for_ten_seconds_and_idle_after_sixty() {
    use meldframe_activity::LaneState::*;
    let mut a = activity();
    a.observe_op(&op(1, "agent:a", "k:0", set_x("a", 1.0)), 100_000);
    for (now, want) in [
        (109_999, Working),
        (110_000, Quiet),
        (159_999, Quiet),
        (160_000, Idle),
    ] {
        a.tick(now);
        assert_eq!(a.lanes()[0].state, want, "at {now}");
    }
    // A touch counts as activity, and none is a ring for the person's own actor.
    a.observe_touch("agent:a", &["a".into()], 161_000);
    a.observe_touch(ME, &["a".into()], 161_000);
    assert_eq!(
        a.lanes()
            .iter()
            .find(|l| l.actor == "agent:a")
            .unwrap()
            .state,
        Working
    );
    assert_eq!(a.rings().len(), 1);
    a.tick(161_000 + 1_499);
    assert_eq!(a.rings().len(), 1);
    a.tick(161_000 + 1_500);
    assert!(a.rings().is_empty());
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
    let lane = |a: &meldframe_activity::Activity| {
        let l = a.lanes().remove(0);
        (l.ops, l.runs, l.last_at_ms)
    };
    let before = (a.rows(), lane(&a));
    for o in &ops {
        a.observe_op(o, 50_000);
    }
    // The clock moved on, but a replay is no activity: the lane's last activity stays where it was.
    assert_eq!((a.rows(), lane(&a)), before);
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
