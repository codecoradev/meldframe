//! A count that cannot be known is not zero (rule U9, finding T19 of the thin-UI audit): the server's count of batches
//! nobody accepted is unknown until a batch frame or a page of the list brings it, and the number of commands of a batch
//! the app only knows from its ops is unknown too.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::*;
use meldframe_activity::{Feed, Page, Status};
use meldframe_revert::ConfirmedLog;

#[test]
fn unreviewed_count_is_unknown_until_the_first_list_has_loaded() {
    let mut a = activity();
    assert_eq!(a.unreviewed_count(), None, "nothing was read yet");
    // An op alone says nothing about the server's count.
    a.observe_op(&op(1, "agent:a", "k:0", add("r", "Card", None)), 1_000);
    assert_eq!(a.unreviewed_count(), None);
    // A list with no batch in it is an answer: nothing waits.
    a.observe_page([], 0);
    assert_eq!(a.unreviewed_count(), Some(0));
}

#[test]
fn a_batch_frame_brings_the_count_too() {
    let mut a = activity();
    a.observe_header(header(1, 1, "agent:a", "k1", 1, 1_000), 3);
    assert_eq!(a.unreviewed_count(), Some(3));
}

#[test]
fn a_page_of_rows_that_all_fail_the_check_is_a_bad_answer_and_says_nothing() {
    let mut a = activity();
    let mut bad = header(1, 1, "agent:a", "k1", 1, 1_000);
    bad.actor = "agent:a\u{202E}".into();
    a.observe_page([bad], 4);
    assert_eq!((a.rows().len(), a.unreviewed_count()), (0, None));
}

#[test]
fn unreviewed_count_is_unknown_after_a_failed_list() {
    let mut feed = Feed::new(ME, 0);
    // The list was asked for and the request failed: nothing was read, so nothing is known.
    assert!(feed.connected(1_000).is_some());
    assert_eq!(feed.failed(), None);
    let log = ConfirmedLog::new(file_before(), 0);
    assert_eq!(feed.view(&log, 0, &[]).unreviewed, None);
}

#[test]
fn a_failed_list_after_a_known_count_leaves_what_the_server_last_said() {
    let mut feed = Feed::new(ME, 0);
    feed.page(Page {
        headers: vec![header(1, 1, "agent:a", "k1", 1, 1_000)],
        unreviewed: 1,
        has_more: false,
        can_review: true,
    });
    feed.failed();
    let log = ConfirmedLog::new(file_before(), 0);
    assert_eq!(feed.view(&log, 0, &[]).unreviewed, Some(1));
}

#[test]
fn starting_again_forgets_the_count() {
    let mut a = activity();
    a.observe_page([], 2);
    a.reset_all(5);
    assert_eq!(a.unreviewed_count(), None);
}

#[test]
fn a_row_made_from_ops_alone_does_not_know_how_many_commands_the_batch_had() {
    let mut a = activity();
    a.observe_op(&op(1, "agent:a", "k:0", add("r", "Card", None)), 1_000);
    let rows = a.rows();
    assert_eq!((rows[0].commands, rows[0].written), (None, 1));
    // The header says it.
    a.observe_header(header(1, 1, "agent:a", "k", 3, 1_000), 1);
    let r = &a.rows()[0];
    assert_eq!(
        (r.commands, r.written, r.status),
        (Some(3), 3, Status::Applied)
    );
}
