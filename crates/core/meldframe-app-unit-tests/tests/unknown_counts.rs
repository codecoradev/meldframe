//! A count that cannot be known is not zero (rule U9, finding T19 of the thin-UI audit), at the app seam: the count of
//! batches nobody accepted is `None` until the server has said it, and stays `None` after a list that failed to load; a
//! batch row made from ops alone has no command count.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, Effect, Frame};
use meldframe_app_model::{BatchHeader, BatchPage, ReviewState};
use meldframe_protocol::{Command, NodePatch};

const T: u64 = 1_700_000_000_000;

fn header(n: u32, rev: u64, commands: u32) -> BatchHeader {
    BatchHeader {
        n,
        rev,
        actor: "agent:x".into(),
        key_prefix: format!("b{n}"),
        tool: "apply_commands".into(),
        commands,
        written: commands,
        atomic: false,
        first_seq: 1,
        last_seq: u64::from(commands),
        at_ms: T,
        note: None,
        review: ReviewState::Unreviewed,
        reviewed_by: None,
        reviewed_at_ms: None,
    }
}

fn count(l: &Loopback) -> Option<u32> {
    l.app.state().activity.unreviewed_count
}

fn fetches_newest(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::FetchBatches { .. }))
}

#[test]
fn unreviewed_count_is_unknown_until_the_first_list_has_loaded() {
    let mut l = Loopback::live();
    assert_eq!(count(&l), None, "a new app has read nothing");
    let fx = l.app.dispatch(Action::OpenActivity {});
    assert!(fetches_newest(&fx));
    assert_eq!(count(&l), None, "asking is not an answer");
    l.app.dispatch(Action::BatchesLoaded {
        page: BatchPage {
            batches: vec![],
            has_more: false,
            unreviewed_count: 0,
            can_review: true,
        },
    });
    assert_eq!(count(&l), Some(0), "an empty list says nothing waits");
}

#[test]
fn unreviewed_count_is_unknown_after_a_failed_list() {
    let mut l = Loopback::live();
    l.app.dispatch(Action::OpenActivity {});
    l.app.dispatch(Action::BatchRequestFailed {
        code: "unavailable".into(),
    });
    assert_eq!(count(&l), None, "a list that did not load says nothing");
    // The next list that does load says it.
    l.app.dispatch(Action::BatchesLoaded {
        page: BatchPage {
            batches: vec![header(1, 1, 2)],
            has_more: false,
            unreviewed_count: 1,
            can_review: false,
        },
    });
    assert_eq!(count(&l), Some(1));
}

#[test]
fn a_known_count_stays_when_a_later_list_fails_to_load() {
    // The doc of `ActivityState::unreviewed_count`: unknown only until a count has been said; a failed list after that
    // keeps the last number the server gave.
    let mut l = Loopback::live();
    l.app.dispatch(Action::Frame {
        now_ms: T,
        frame: Frame::Batch {
            header: header(1, 1, 1),
            unreviewed_count: 3,
        },
    });
    l.app.dispatch(Action::OpenActivity {});
    l.app.dispatch(Action::BatchRequestFailed {
        code: "unavailable".into(),
    });
    assert_eq!(count(&l), Some(3));
}

#[test]
fn a_batch_frame_gives_the_count_and_a_reload_forgets_it() {
    let mut l = Loopback::live();
    let frame = Frame::Batch {
        header: header(1, 1, 1),
        unreviewed_count: 3,
    };
    l.app.dispatch(Action::Frame { now_ms: T, frame });
    assert_eq!(count(&l), Some(3));
    l.app.dispatch(Action::Reload {
        now_ms: T,
        nodes: doc(),
        variables: Default::default(),
        seq: l.seq,
        applied: vec![],
    });
    assert_eq!(
        count(&l),
        None,
        "the numbering starts again: nothing is known"
    );
}

#[test]
fn a_batch_row_with_no_header_does_not_know_its_command_count() {
    let mut l = Loopback::live();
    let patch = NodePatch {
        x: Some(5.0),
        ..Default::default()
    };
    let command = Command::UpdateNode {
        id: "r1".into(),
        patch,
    };
    l.remote("agent:x", "b1:0", T, command);
    let row = &l.app.state().activity.rows[0];
    assert_eq!((row.n, row.commands, row.written), (None, None, 1));
    // The header arrives and says it.
    let frame = Frame::Batch {
        header: header(1, 1, 4),
        unreviewed_count: 1,
    };
    l.app.dispatch(Action::Frame { now_ms: T, frame });
    let row = &l.app.state().activity.rows[0];
    assert_eq!((row.n, row.commands), (Some(1), Some(4)));
}
