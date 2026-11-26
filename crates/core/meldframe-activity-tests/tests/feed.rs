//! `Feed` and what the first review of slice 9 found in the reducer: `revertable` for a batch with nothing to show, a
//! page that must not overwrite a newer count, rule (a) at most once in 5 seconds, a clock that runs while nothing can be
//! asked, rule (d)'s chain and the memo of `inEffect`.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::*;
use meldframe_activity::{Feed, Fetch, InEffect, Page, Status};
use meldframe_revert::ConfirmedLog;

fn log(base_seq: u64, ops: Vec<meldframe_revert::LoggedOp>) -> ConfirmedLog {
    let mut l = ConfirmedLog::new(file_before(), base_seq);
    ops.into_iter().for_each(|o| l.push(o));
    l
}

fn page(ns: &[u32], has_more: bool) -> Page {
    let headers = ns
        .iter()
        .map(|n| header(*n, u64::from(*n), "agent:a", &format!("p{n}"), 1, 1_000))
        .collect();
    Page {
        headers,
        unreviewed: 1,
        has_more,
        can_review: true,
    }
}

#[test]
fn a_batch_with_nothing_to_show_is_revertable_and_a_header_only_row_is_not() {
    let mut feed = Feed::new(ME, 0);
    let l = log(0, vec![op(1, "agent:a", "del:0", remove("a"))]);
    feed.red.observe_op(&l.ops()[0], 5_000);
    feed.red
        .observe_header(header(9, 1, "agent:z", "zz", 1, 5_000), 1);
    let v = feed.view(&l, 1, &[]);
    let by = |p: &str| v.rows.iter().find(|r| r.row.key_prefix == p).unwrap();
    assert!(!by("del").row.can_show, "a delete alone shows nothing");
    assert!(by("del").revertable);
    assert!(!by("zz").revertable, "a header alone has no ops to revert");
}

#[test]
fn a_page_changes_the_count_only_with_a_strictly_higher_rev_and_a_row_never() {
    let mut a = activity();
    a.observe_header(header(1, 1, "agent:a", "k1", 1, 1_000), 1);
    a.observe_header(header(1, 2, "agent:a", "k1", 1, 1_000), 0);
    a.observe_page([header(1, 2, "agent:a", "k1", 1, 1_000)], 1);
    assert_eq!(
        a.unreviewed_count(),
        Some(0),
        "a page as new as the count does not overwrite it"
    );
    a.observe_page([header(1, 3, "agent:a", "k1", 1, 1_000)], 1);
    assert_eq!(a.unreviewed_count(), Some(1));
    a.observe_row(header(2, 9, "agent:a", "k2", 1, 1_000));
    assert_eq!(
        (a.rows().len(), a.unreviewed_count(), a.highest_rev()),
        (2, Some(1), 3)
    );
}

#[test]
fn rule_a_asks_at_most_once_in_five_seconds() {
    let mut a = activity();
    a.observe_header(header(1, 1, "agent:a", "k1", 1, 1_000), 1);
    a.clock(10_000);
    assert!(
        a.observe_header(header(3, 3, "agent:a", "k3", 1, 1_000), 1)
            .is_some()
    );
    a.clock(11_000);
    assert_eq!(
        a.observe_header(header(6, 6, "agent:a", "k6", 1, 1_000), 1),
        None
    );
    a.clock(15_000);
    assert!(
        a.observe_header(header(9, 9, "agent:a", "k9", 1, 1_000), 1)
            .is_some()
    );
}

#[test]
fn a_clock_with_no_connection_spends_no_try_and_a_live_one_asks() {
    let mut feed = Feed::new(ME, 0);
    feed.red
        .observe_op(&op(1, "agent:a", "lost:0", set_x("a", 1.0)), 10_000);
    for at in [14_000, 20_000, 26_000, 32_000] {
        assert_eq!(feed.tick(at, false), None);
    }
    let status = |f: &Feed| f.red.rows()[0].status;
    assert_eq!(
        status(&feed),
        Status::Building,
        "nothing was asked, so no try was spent"
    );
    assert!(feed.tick(33_000, true).is_some());
    assert_eq!(status(&feed), Status::Building);
}

/// A page of `n` headers, `hi` down to `hi - n + 1`.
fn run(hi: u32, n: u32, has_more: bool) -> Page {
    let ns: Vec<u32> = (0..n).map(|i| hi - i).collect();
    page(&ns, has_more)
}

#[test]
fn rule_d_goes_down_only_on_the_page_it_waits_for_and_stops_at_the_floor() {
    let mut feed = Feed::new(ME, 0);
    feed.red.observe_page(page(&[5], false).headers, 1);
    assert_eq!(feed.connected(1_000), Some(Fetch::NEWEST));
    // Another rule's small page above the highest `n` held is not the newest page: the chain neither ends nor moves.
    assert_eq!(feed.page(page(&[9, 8], false)), None);
    assert!(feed.healing());
    assert_eq!(feed.page(run(120, 100, true)), Some(Fetch::before(21)));
    // A page that is not below 21 is not the answer to that request.
    assert_eq!(feed.page(page(&[30, 29], true)), None);
    assert!(feed.healing());
    assert_eq!(feed.page(run(20, 10, true)), Some(Fetch::before(11)));
    assert_eq!(feed.page(run(10, 6, true)), None, "it reached the floor");
    assert!(!feed.healing(), "and the chain has ended");
    assert_eq!(feed.page(run(4, 2, true)), None);
}

#[test]
fn a_reconnect_with_the_small_answer_of_rule_b_first_still_reads_down_to_the_floor() {
    let mut feed = Feed::new(ME, 0);
    feed.red.observe_page(page(&[5], false).headers, 1);
    feed.connected(1_000);
    // The answer of a fetch of rule (b) (n above the highest held) arrives before the newest page.
    assert_eq!(feed.page(page(&[7], false)), None);
    assert_eq!(feed.page(run(150, 100, true)), Some(Fetch::before(51)));
    assert!(feed.healing());
}

#[test]
fn rule_d_stops_after_five_pages_and_on_a_failure_and_an_empty_page_below() {
    let mut feed = Feed::new(ME, 0);
    feed.connected(1_000);
    let asked = (0..10u32)
        .take_while(|i| feed.page(run(900 - i * 100, 100, true)).is_some())
        .count();
    assert_eq!(asked, 4, "five pages in all, the first included");
    feed.connected(2_000);
    assert_eq!(feed.failed(), None);
    assert!(!feed.healing(), "a failure ends the chain");
    feed.connected(3_000);
    feed.page(run(900, 100, true));
    assert_eq!(feed.page(page(&[], false)), None);
    assert!(!feed.healing(), "an empty page below ends it");
}

#[test]
fn a_subscribe_takes_the_window_so_rule_b_does_not_ask_again_at_once() {
    let mut feed = Feed::new(ME, 0);
    feed.red
        .observe_op(&op(1, "agent:a", "lost:0", set_x("a", 1.0)), 10_000);
    assert!(feed.connected(14_000).is_some());
    assert_eq!(
        feed.tick(14_500, true),
        None,
        "inside the window of the fetch rule (d) asked for"
    );
    assert_eq!(feed.tick(18_999, true), None);
    assert!(
        feed.tick(19_000, true).is_some(),
        "once the window is over rule (b) asks, and spends a try"
    );
}

#[test]
fn a_gap_inside_the_window_is_asked_for_when_the_window_is_over() {
    let mut a = activity();
    a.observe_header(header(1, 1, "agent:a", "k1", 1, 1_000), 1);
    a.clock(10_000);
    assert!(
        a.observe_header(header(3, 3, "agent:a", "k3", 1, 1_000), 1)
            .is_some()
    );
    a.clock(11_000);
    assert_eq!(
        a.observe_header(header(6, 6, "agent:a", "k6", 1, 1_000), 1),
        None
    );
    assert_eq!(a.tick(14_000), None, "the window is not over");
    let newest = a.tick(15_000).expect("the lost gap is asked for");
    assert_eq!(
        (newest.before_n, newest.after_n),
        (None, None),
        "the newest page: the hole may be an older row"
    );
    assert_eq!(a.tick(30_000), None, "once");
}

#[test]
fn the_memo_of_in_effect_follows_the_record_and_a_reset_clears_it() {
    let mut feed = Feed::new(ME, 0);
    let l = log(0, vec![op(1, "agent:a", "b:0", set_x("a", 5.0))]);
    assert_eq!(feed.in_effect(&l, 1, "agent:a", "b"), InEffect::Yes);
    // Same number, same count of ops, but a record that starts elsewhere: not the old answer.
    let other = log(3, vec![op(4, "agent:b", "c:0", set_x("a", 1.0))]);
    assert_eq!(feed.in_effect(&other, 1, "agent:a", "b"), InEffect::Unknown);
    // The key cannot see a record rewritten under it (same start, same count): the memo answers as it did, until a
    // reset clears it.
    let rewritten = log(0, vec![op(1, "agent:b", "b:0", set_x("a", 0.0))]);
    feed.in_effect(&l, 1, "agent:a", "b");
    assert_eq!(
        feed.in_effect(&rewritten, 1, "agent:a", "b"),
        InEffect::Yes,
        "the memo"
    );
    feed.reset(1);
    assert_ne!(
        feed.in_effect(&rewritten, 1, "agent:a", "b"),
        InEffect::Yes,
        "a reset drops the memo"
    );
    assert_eq!(feed.red.rows().len(), 0, "and starts the reducer again");
}
