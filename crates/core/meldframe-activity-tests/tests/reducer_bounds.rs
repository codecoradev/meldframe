//! The bounds of the app state: 32 lanes, 500 rows (the oldest goes first), and a revert that changes neither the
//! review state nor the count (ADR-0021, "What the app layer owns" and "Counters").

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::*;
use meldframe_activity::{MAX_LANES, MAX_ROWS, Review};
use meldframe_revert::{Batch, revert_batch};

#[test]
fn the_501st_row_drops_the_oldest() {
    assert_eq!((MAX_LANES, MAX_ROWS), (32, 500));
    let mut a = activity();
    for i in 1..=500u64 {
        let actor = format!("agent:{}", i % 10);
        let prefix = format!("k{i}");
        a.observe_op(
            &op(i, &actor, &format!("{prefix}:0"), set_x("a", i as f64)),
            i,
        );
        a.observe_header(header(i as u32, i, &actor, &prefix, 1, i), i as u32);
    }
    assert_eq!(a.rows().len(), 500);
    assert_eq!(a.rows().last().unwrap().n, Some(1));
    a.observe_op(&op(501, "agent:1", "k501:0", set_x("a", 0.0)), 501);
    a.observe_header(header(501, 501, "agent:1", "k501", 1, 501), 501);
    let rows = a.rows();
    assert_eq!(rows.len(), 500);
    assert_eq!((rows[0].n, rows.last().unwrap().n), (Some(501), Some(2)));
    // The count is the server's, not the number of rows held.
    assert_eq!(a.unreviewed_count(), Some(501));
    // An older page that is past the bound is dropped again at once.
    a.observe_page([header(1, 1, "agent:1", "k1", 1, 1)], 501);
    assert_eq!(a.rows().last().unwrap().n, Some(2));
    assert_eq!(a.lowest_n(), Some(2));
}

#[test]
fn the_33rd_lane_drops_the_lane_with_the_oldest_activity_and_its_rows() {
    let mut a = activity();
    for i in 0..33u64 {
        let actor = format!("agent:{i}");
        a.observe_op(&op(i + 1, &actor, "k:0", set_x("a", 1.0)), 1_000 + i);
        a.observe_header(header(i as u32 + 1, i + 1, &actor, "k", 1, 1_000 + i), 33);
    }
    let lanes = a.lanes();
    assert_eq!(lanes.len(), 32);
    assert_eq!(lanes[0].actor, "agent:32");
    assert!(lanes.iter().all(|l| l.actor != "agent:0"));
    assert_eq!(a.rows().len(), 32);
    assert!(a.rows().iter().all(|r| r.actor != "agent:0"));
}

#[test]
fn a_revert_leaves_the_review_state_and_the_count_as_they_were() {
    let before = file_before();
    let mut a = activity();
    let ops = vec![op(1, "agent:a", "k:0", set_x("a", 50.0))];
    for o in &ops {
        a.observe_op(o, 1_000);
    }
    let mut h = header(1, 3, "agent:a", "k", 1, 1_000);
    h.review = Review::NeedsReview;
    h.reviewed_by = Some(ME.into());
    h.reviewed_at_ms = Some(900);
    a.observe_header(h, 1);
    let rows_before = a.rows();

    // The person reverts it: new ops of the person's own, with editor keys.
    let r = revert_batch(
        &before,
        &ops,
        Batch {
            actor: "agent:a",
            key_prefix: "k",
        },
    )
    .unwrap();
    assert_eq!(r.commands.len(), 1);
    for (i, c) in r.commands.into_iter().enumerate() {
        a.observe_op(&op(10 + i as u64, ME, &format!("ed-rv-{i}"), c), 2_000);
    }
    let rows = a.rows();
    assert_eq!(rows, rows_before);
    assert_eq!(
        (rows[0].review, rows[0].reviewed_by.as_deref()),
        (Review::NeedsReview, Some(ME))
    );
    assert_eq!(a.unreviewed_count(), Some(1));
}
