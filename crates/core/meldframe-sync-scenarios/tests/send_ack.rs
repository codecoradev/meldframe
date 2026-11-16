//! The send and acknowledge path of a `Replica`: a local command is applied at once, queued as an op
//! with a stable key, and confirmed when the server returns it. No other actor takes part here.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-sync/tests/common/mod.rs"]
mod common;

use common::{add, base, snap, upd, x_of};
use meldframe_sync::{EventOutcome, OpKind, Replica, ReplicaError};

fn replica() -> Replica {
    Replica::new("user:a", "ca", base(), 0).unwrap()
}

#[test]
fn a_local_command_shows_at_once_and_waits_as_a_pending_op() {
    let mut r = replica();
    r.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    assert_eq!(x_of(r.nodes(), "a"), 10.0, "the view has the edit");
    assert_eq!(x_of(r.base(), "a"), 0.0, "the confirmed state does not yet");
    assert_eq!(r.pending_len(), 1);
    let out = r.take_outbox();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].key, "ca-1");
    assert_eq!(out[0].kind, OpKind::Edit);
    assert_eq!(out[0].seen_seq, 0);
    assert!(r.take_outbox().is_empty(), "the outbox is handed over once");
    r.check_consistency().unwrap();
}

#[test]
fn a_refused_command_queues_nothing() {
    let mut r = replica();
    assert!(r.apply(&upd("missing", |p| p.x = Some(1.0))).is_err());
    assert_eq!(r.pending_len(), 0);
    assert!(r.take_outbox().is_empty());
    assert_eq!(snap(r.nodes()), snap(r.base()));
}

#[test]
fn the_server_returning_my_op_confirms_it_in_order() {
    let mut r = replica();
    r.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    r.apply(&upd("a", |p| p.y = Some(2.0))).unwrap();
    let out = r.take_outbox();
    assert_eq!(
        r.on_event(1, "user:a", &out[0].command),
        Ok(EventOutcome::Confirmed)
    );
    assert_eq!((r.seq(), r.pending_len()), (1, 1));
    assert_eq!(x_of(r.base(), "a"), 1.0);
    assert_eq!(
        r.on_event(2, "user:a", &out[1].command),
        Ok(EventOutcome::Confirmed)
    );
    assert_eq!((r.seq(), r.pending_len()), (2, 0));
    assert_eq!(snap(r.base()), snap(r.nodes()));
    r.check_consistency().unwrap();
}

#[test]
fn an_event_seen_before_is_ignored_and_a_gap_is_reported() {
    let mut r = replica();
    r.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    let out = r.take_outbox();
    r.on_event(1, "user:a", &out[0].command).unwrap();
    assert_eq!(
        r.on_event(1, "user:a", &out[0].command),
        Ok(EventOutcome::Duplicate)
    );
    assert_eq!(
        r.on_event(5, "user:a", &out[0].command),
        Err(ReplicaError::Gap {
            expected: 2,
            got: 5
        })
    );
    assert_eq!(r.seq(), 1, "a gap changes nothing");
}

#[test]
fn unconfirmed_ops_can_be_sent_again_with_the_same_keys() {
    let mut r = replica();
    r.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    r.apply(&add("n", "rect", Some("a"))).unwrap();
    let sent = r.take_outbox();
    let again = r.unconfirmed();
    let keys = |v: &[meldframe_sync::Outgoing]| v.iter().map(|o| o.key.clone()).collect::<Vec<_>>();
    assert_eq!(keys(&again), keys(&sent));
    r.on_event(1, "user:a", &sent[0].command).unwrap();
    assert_eq!(
        keys(&r.unconfirmed()),
        ["ca-2"],
        "a confirmed op is not sent again"
    );
}

#[test]
fn undo_and_redo_travel_as_ordinary_ops_one_per_command() {
    let mut r = replica();
    r.begin_group();
    r.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    r.apply(&upd("k", |p| p.x = Some(2.0))).unwrap();
    r.end_group();
    for (i, o) in r.take_outbox().iter().enumerate() {
        r.on_event(i as u64 + 1, "user:a", &o.command).unwrap();
    }
    assert!(r.can_undo() && !r.can_redo());
    assert!(r.undo().unwrap());
    let undo = r.take_outbox();
    assert_eq!(undo.len(), 2, "one op for each command of the step");
    assert!(undo.iter().all(|o| o.kind == OpKind::Undo));
    assert_eq!((x_of(r.nodes(), "a"), x_of(r.nodes(), "k")), (0.0, 0.0));
    assert!(r.can_redo() && !r.can_undo());
    assert!(r.redo().unwrap());
    let redo = r.take_outbox();
    assert!(redo.iter().all(|o| o.kind == OpKind::Redo));
    assert_eq!(x_of(r.nodes(), "k"), 2.0);
    r.check_consistency().unwrap();
}

#[test]
fn undo_with_nothing_to_undo_sends_nothing() {
    let mut r = replica();
    assert_eq!(r.undo(), Ok(false));
    assert_eq!(r.redo(), Ok(false));
    assert!(r.take_outbox().is_empty());
}
