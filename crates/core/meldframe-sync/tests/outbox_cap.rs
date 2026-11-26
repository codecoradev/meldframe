//! The queue of pending ops has a cap (issue 90): a new local op is refused with a stable code when the
//! queue is full, instead of the queue growing without bound.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "common/mod.rs"]
mod common;

use common::{base, snap, upd, x_of};
use meldframe_sync::{DEFAULT_MAX_PENDING, Replica, SubmitError};

const ACTOR: &str = "user:a";

fn full(max: usize) -> Replica {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.set_max_pending(max);
    for i in 0..max {
        r.apply(&upd("a", |p| p.x = Some(i as f64))).unwrap();
    }
    r
}

#[test]
fn the_default_cap_is_set_and_can_be_changed() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    assert_eq!(r.max_pending(), DEFAULT_MAX_PENDING);
    const { assert!(DEFAULT_MAX_PENDING >= 1_000) };
    r.set_max_pending(7);
    assert_eq!(r.max_pending(), 7);
}

#[test]
fn a_local_op_beyond_the_cap_is_refused_and_changes_nothing() {
    let mut r = full(3);
    let (view, out, undo) = (snap(r.nodes()), r.unconfirmed().len(), r.undo_depth());
    let err = r.apply(&upd("a", |p| p.x = Some(99.0))).unwrap_err();
    assert_eq!(err, SubmitError::OutboxFull { max: 3 });
    assert_eq!(err.code(), "E_OUTBOX_FULL");
    assert!(err.to_string().contains('3'), "{err}");
    assert_eq!(
        (snap(r.nodes()), r.unconfirmed().len(), r.undo_depth()),
        (view, out, undo)
    );
    assert_eq!(r.pending_len(), 3);
    assert_eq!(
        r.take_outbox().len(),
        3,
        "nothing was queued by the refused op"
    );
    r.check_consistency().unwrap();
}

#[test]
fn undo_and_redo_are_refused_when_the_queue_is_full() {
    let mut r = full(2);
    assert_eq!(r.undo().unwrap_err(), SubmitError::OutboxFull { max: 2 });
    assert!(!r.redo().unwrap(), "nothing to redo is not an error");
    assert_eq!(r.pending_len(), 2);
    assert_eq!(
        r.undo_depth(),
        2,
        "a refused undo does not move the history"
    );
    assert_eq!(x_of(r.nodes(), "a"), 1.0);
}

#[test]
fn undo_and_redo_with_nothing_to_undo_are_not_an_error_at_the_cap() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.set_max_pending(0);
    assert!(!r.undo().unwrap());
    assert!(!r.redo().unwrap());
}

#[test]
fn an_invalid_command_still_gives_its_own_error_code() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    let err = r.apply(&upd("missing", |p| p.x = Some(1.0))).unwrap_err();
    assert!(matches!(err, SubmitError::Invalid(_)), "{err:?}");
    assert_eq!(err.code(), "E_NODE_NOT_FOUND");
}

#[test]
fn room_frees_up_as_the_server_confirms_ops() {
    let mut r = full(2);
    let ops = r.take_outbox();
    r.on_event_keyed(1, ACTOR, Some(&ops[0].key), &ops[0].command)
        .unwrap();
    r.apply(&upd("a", |p| p.x = Some(50.0))).unwrap();
    assert!(r.apply(&upd("a", |p| p.x = Some(51.0))).is_err());
    // A refusal by the server frees room as well.
    assert!(r.on_reject(&ops[1].key));
    r.apply(&upd("a", |p| p.x = Some(52.0))).unwrap();
}

#[test]
fn the_cap_code_is_in_the_error_code_docs() {
    const DOC: &str = include_str!("../../../../docs/error-codes.md");
    let section = DOC.split("\n## Replica codes\n").nth(1).expect("section");
    let section = section.split("\n## ").next().unwrap();
    let rows: Vec<&str> = section
        .lines()
        .filter(|l| l.starts_with("| `"))
        .map(|l| l[3..].split('`').next().unwrap())
        .collect();
    assert_eq!(rows, ["E_OUTBOX_FULL", "E_SNAPSHOT_BEHIND"]);
    // The app layer words the same refusal with its own code, and the docs must name what exists.
    assert!(section.contains("`Event::Refused`") && section.contains("`outbox_full`"));
    assert!(!section.contains("CommandRejected") && !section.contains("`Submit`"));
}
