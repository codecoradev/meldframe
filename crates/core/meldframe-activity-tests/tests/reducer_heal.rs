//! A header the live stream lost is healed by a rev gap (a) or by the wait for a prefix with no header (b), and a
//! prefix that stays headerless ends as `headerless` (ADR-0021, "Live").

// Under WASM these tests will run through wasm-bindgen-test once `just wasm-test` lists this crate (slice 10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::*;
use meldframe_activity::{Fetch, Status};

fn newest(after_n: Option<u32>) -> Fetch {
    Fetch {
        before_n: None,
        after_n,
        limit: 100,
    }
}

/// Header 3 is held; the ops of prefix `lost` arrived at 10 s and no header came.
fn lost_header() -> meldframe_activity::Activity {
    let mut a = activity();
    a.observe_header(header(3, 3, "agent:a", "held", 1, 1_000), 1);
    a.observe_op(&op(40, "agent:a", "lost:0", set_x("a", 1.0)), 10_000);
    a.observe_op(&op(41, "agent:a", "lost:1", set_x("a", 2.0)), 10_100);
    a
}

fn status_of(a: &meldframe_activity::Activity, prefix: &str) -> Status {
    a.rows()
        .into_iter()
        .find(|r| r.key_prefix == prefix)
        .unwrap()
        .status
}

#[test]
fn a_rev_more_than_one_above_the_highest_asks_for_the_newest_page() {
    let mut a = activity();
    assert_eq!(
        a.observe_header(header(1, 1, "agent:a", "k1", 1, 1), 1),
        None
    );
    assert_eq!(
        a.observe_header(header(2, 2, "agent:a", "k2", 1, 2), 2),
        None
    );
    // Rev 3 and 4 were lost.
    assert_eq!(
        a.observe_header(header(5, 5, "agent:a", "k5", 1, 5), 4),
        Some(newest(None))
    );
    // The gap is asked for once; the next rev in line is no gap, and a late older one is none either.
    assert_eq!(
        a.observe_header(header(6, 6, "agent:a", "k6", 1, 6), 5),
        None
    );
    assert_eq!(
        a.observe_header(header(3, 3, "agent:a", "k3", 1, 3), 3),
        None
    );
}

#[test]
fn a_prefix_with_no_header_is_building_then_fetched_after_three_seconds() {
    let mut a = lost_header();
    assert_eq!(status_of(&a, "lost"), Status::Building);
    assert_eq!(a.tick(12_999), None);
    // After 3 s from its first op: fetch what is newer than the highest n held.
    assert_eq!(a.tick(13_000), Some(newest(Some(3))));
    // The header arrives (the answer of the fetch): the row is final and nothing more is asked.
    assert_eq!(
        a.observe_header(header(4, 4, "agent:a", "lost", 2, 10_050), 2),
        None
    );
    assert_eq!(status_of(&a, "lost"), Status::Applied);
    assert_eq!(a.rows()[0].n, Some(4));
    assert_eq!(a.tick(40_000), None);
}

#[test]
fn at_most_one_fetch_in_five_seconds_and_three_tries_then_headerless_until_a_header_comes() {
    let mut a = lost_header();
    assert_eq!(a.tick(13_000), Some(newest(Some(3))));
    assert_eq!(a.tick(17_999), None);
    assert_eq!(a.tick(18_000), Some(newest(Some(3))));
    assert_eq!(status_of(&a, "lost"), Status::Building);
    assert_eq!(a.tick(23_000), Some(newest(Some(3))));
    // Three tries are spent: final, though the lane is still working.
    assert_eq!(status_of(&a, "lost"), Status::Headerless);
    assert_eq!(a.tick(30_000), None);
    assert_eq!(a.tick(300_000), None);
    let r = a
        .rows()
        .into_iter()
        .find(|r| r.key_prefix == "lost")
        .unwrap();
    assert!((r.n, r.rev, r.note.is_none()) == (None, 0, true));
    assert_eq!(r.review, meldframe_activity::Review::Unreviewed);
    assert_eq!(r.written, 2);
    // It is not counted by the server's number.
    assert_eq!(a.unreviewed_count(), 1);
    // A header that comes late still turns it into a numbered row.
    a.observe_header(header(4, 4, "agent:a", "lost", 2, 10_050), 2);
    assert_eq!(status_of(&a, "lost"), Status::Applied);
}

#[test]
fn a_lane_that_goes_idle_makes_its_headerless_rows_final() {
    let mut a = lost_header();
    assert_eq!(a.tick(13_000), Some(newest(Some(3))));
    assert_eq!(status_of(&a, "lost"), Status::Building);
    // 60 s after its last op the lane is idle; one try is left, but the row is final and no more is asked.
    assert_eq!(a.tick(70_100), None);
    assert_eq!(status_of(&a, "lost"), Status::Headerless);
    // Activity again does not take it back.
    a.observe_op(&op(50, "agent:a", "other:0", set_x("a", 3.0)), 80_000);
    assert_eq!(status_of(&a, "lost"), Status::Headerless);
}

#[test]
fn one_fetch_covers_every_prefix_that_waits_and_counts_a_try_for_each() {
    let mut a = lost_header();
    a.observe_op(&op(42, "agent:b", "lost2:0", set_x("a", 1.0)), 11_000);
    assert_eq!(a.tick(13_000), Some(newest(Some(3))));
    // lost2 has waited 2 s only: not covered yet, so it is asked for with the next fetch.
    assert_eq!(a.tick(18_000), Some(newest(Some(3))));
    assert_eq!(a.tick(23_000), Some(newest(Some(3))));
    assert_eq!(status_of(&a, "lost"), Status::Headerless);
    assert_eq!(status_of(&a, "lost2"), Status::Building);
    assert_eq!(a.tick(28_000), Some(newest(Some(3))));
    assert_eq!(status_of(&a, "lost2"), Status::Headerless);
    assert_eq!(a.tick(40_000), None);
}

#[test]
fn a_page_of_headers_is_taken_in_without_asking_for_a_fetch() {
    let mut a = activity();
    a.observe_page(
        [
            header(1, 4, "agent:a", "k1", 1, 1),
            header(2, 9, "agent:a", "k2", 1, 2),
        ],
        1,
    );
    assert_eq!((a.lowest_n(), a.highest_n()), (Some(1), Some(2)));
    assert_eq!(a.highest_rev(), 9);
    assert_eq!(a.unreviewed_count(), 1);
    assert_eq!(
        a.observe_header(header(3, 10, "agent:a", "k3", 1, 3), 2),
        None
    );
}
