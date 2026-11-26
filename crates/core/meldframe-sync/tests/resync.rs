//! Resync after `Diverged` (issue 90): load the file again (a new snapshot) and replay the pending ops
//! with their keys and dependencies, instead of reloading the page.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "common/mod.rs"]
mod common;

use common::{base, node, remove, snap, upd, x_of};
use meldframe_sync::{EventOutcome, Replica, ReplicaError, ResyncError};

const ACTOR: &str = "user:a";

/// A replica with one confirmed edit and one pending edit, that has then diverged: another actor's event
/// does not apply to its confirmed state.
fn diverged() -> Replica {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    let e = r.take_outbox();
    r.on_event_keyed(1, ACTOR, Some(&e[0].key), &e[0].command)
        .unwrap();
    r.apply(&upd("a", |p| p.y = Some(7.0))).unwrap();
    let ghost = upd("ghost", |p| p.x = Some(1.0));
    let err = r.on_event(2, "user:b", &ghost).unwrap_err();
    assert!(matches!(err, ReplicaError::Diverged(_)), "{err:?}");
    r
}

/// The file as the server holds it at seq 4: my edit x = 5 and another actor's change.
fn snapshot() -> Vec<meldframe_protocol::Node> {
    let mut n = base();
    n.iter_mut().find(|n| n.id == "a").unwrap().x = 5.0;
    n.iter_mut().find(|n| n.id == "k").unwrap().x = 9.0;
    n
}

#[test]
fn a_diverged_replica_resyncs_from_a_new_snapshot_and_keeps_its_pending_ops() {
    let mut r = diverged();
    let pending = r.unconfirmed();
    assert_eq!(pending.len(), 1);
    assert_eq!(r.resync(snapshot(), 4, &[]).unwrap(), 0);
    assert_eq!(r.seq(), 4);
    assert_eq!((x_of(r.base(), "a"), x_of(r.base(), "k")), (5.0, 9.0));
    r.check_consistency().unwrap();
    // The pending op is still there with its key, and the view is the new base plus it.
    let again = r.unconfirmed();
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].key, pending[0].key);
    assert_eq!(x_of(r.nodes(), "k"), 9.0);
    // It is queued to be sent again.
    assert_eq!(r.take_outbox().len(), 1);
    // The stream goes on from the snapshot: the op comes back by its key and is confirmed.
    let got = r.on_event_keyed(5, ACTOR, Some(&pending[0].key), &pending[0].command);
    assert_eq!(got.unwrap(), EventOutcome::Confirmed);
    assert_eq!(r.pending_len(), 0);
}

#[test]
fn ops_the_snapshot_already_holds_are_confirmed_by_their_key() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    r.apply(&upd("a", |p| p.y = Some(7.0))).unwrap();
    let ops = r.take_outbox();
    let mut snap_nodes = snapshot();
    snap_nodes.iter_mut().find(|n| n.id == "a").unwrap().y = 7.0;
    // The server applied both before the snapshot: neither comes back as an event.
    let keys: Vec<String> = ops.iter().map(|o| o.key.clone()).collect();
    r.resync(snap_nodes, 4, &keys).unwrap();
    assert_eq!(r.pending_len(), 0);
    assert!(r.take_outbox().is_empty());
    r.check_consistency().unwrap();
    assert_eq!(x_of(r.nodes(), "a"), 5.0);
}

#[test]
fn the_undo_history_starts_empty_and_no_replayed_op_is_an_undo_step() {
    // Another actor's writes between the old seq and the snapshot were not followed, so the history made
    // before cannot be rewritten against them (an undo would overwrite their write): it is dropped.
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    let e = r.take_outbox();
    r.on_event_keyed(1, ACTOR, Some(&e[0].key), &e[0].command)
        .unwrap();
    assert!(r.can_undo());
    r.apply(&upd("a", |p| p.y = Some(7.0))).unwrap();
    r.resync(snapshot(), 4, &[]).unwrap();
    assert_eq!((r.undo_depth(), r.redo_depth()), (0, 0));
    assert!(!r.can_undo() && !r.can_redo());
    assert_eq!(r.nodes().iter().find(|n| n.id == "a").unwrap().y, 7.0);
    r.check_consistency().unwrap();
}

#[test]
fn pending_undo_and_redo_ops_survive_a_resync_as_they_were_and_add_no_steps() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    let e = r.take_outbox();
    r.on_event_keyed(1, ACTOR, Some(&e[0].key), &e[0].command)
        .unwrap();
    assert!(r.undo().unwrap());
    assert!(r.redo().unwrap());
    assert!(r.undo().unwrap());
    let before = r.unconfirmed();
    let shape = |v: &[meldframe_sync::Outgoing]| {
        v.iter()
            .map(|o| (o.key.clone(), o.kind, o.deps.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(r.resync(snapshot(), 4, &[]).unwrap(), 0);
    assert_eq!(shape(&r.unconfirmed()), shape(&before));
    assert_eq!((r.undo_depth(), r.redo_depth()), (0, 0));
    r.check_consistency().unwrap();
    // A remote event after the resync must not make the replica correct the replayed undo ops.
    let other = upd("k", |p| p.y = Some(1.0));
    r.take_outbox();
    let got = r.on_event(5, "user:b", &other).unwrap();
    assert_eq!(got, EventOutcome::Rebased { dropped: 0 });
    assert!(r.take_outbox().is_empty(), "no correction is sent");
    r.check_consistency().unwrap();
}

#[test]
fn a_snapshot_older_than_the_replica_is_refused_and_changes_nothing() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&upd("a", |p| p.y = Some(7.0))).unwrap();
    r.on_event(1, "user:b", &upd("k", |p| p.x = Some(1.0)))
        .unwrap();
    r.on_event(2, "user:b", &upd("k", |p| p.x = Some(2.0)))
        .unwrap();
    let view = snap(r.nodes());
    let err = r.resync(base(), 1, &[]).unwrap_err();
    assert_eq!(err, ResyncError::Behind { have: 2, got: 1 });
    assert_eq!(err.code(), "E_SNAPSHOT_BEHIND");
    assert_eq!((r.seq(), r.pending_len(), snap(r.nodes())), (2, 1, view));
    assert!(r.resync(base(), 2, &[]).is_ok(), "the same number is fine");
}

#[test]
fn a_duplicate_ack_drains_an_op_the_snapshot_holds_but_the_caller_left_out() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    let ops = r.take_outbox();
    r.resync(snapshot(), 4, &[]).unwrap();
    assert_eq!(r.pending_len(), 1, "replayed: the caller did not name it");
    assert_eq!(r.on_ack("nobody", 3), None, "not pending");
    assert_eq!(r.on_ack(&ops[0].key, 9), None, "its event is still to come");
    assert_eq!(r.pending_len(), 1);
    // The server answers the repeated key with the seq of the first write, which the snapshot holds.
    assert_eq!(r.on_ack(&ops[0].key, 3), Some(0));
    assert_eq!(r.pending_len(), 0);
    assert_eq!(x_of(r.nodes(), "a"), 5.0);
    r.check_consistency().unwrap();
    assert_eq!(r.on_ack(&ops[0].key, 3), None, "only once");
}

#[test]
fn stuck_ops_cannot_exhaust_the_cap_once_their_duplicate_acks_come() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.set_max_pending(2);
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    r.apply(&upd("a", |p| p.y = Some(7.0))).unwrap();
    let ops = r.take_outbox();
    let mut snap_nodes = snapshot();
    snap_nodes.iter_mut().find(|n| n.id == "a").unwrap().y = 7.0;
    r.resync(snap_nodes, 4, &[]).unwrap();
    assert!(r.apply(&upd("k", |p| p.x = Some(1.0))).is_err(), "full");
    for (i, o) in ops.iter().enumerate() {
        assert_eq!(r.on_ack(&o.key, 2 + i as u64), Some(0));
    }
    r.apply(&upd("k", |p| p.x = Some(1.0))).unwrap();
}

#[test]
fn only_the_pending_ops_the_snapshot_lacks_are_sent_again() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    r.apply(&upd("a", |p| p.y = Some(7.0))).unwrap();
    let ops = r.take_outbox();
    r.resync(snapshot(), 4, &[ops[0].key.clone()]).unwrap();
    let left = r.unconfirmed();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].key, ops[1].key);
    assert_eq!(r.take_outbox()[0].key, ops[1].key);
}

#[test]
fn a_pending_op_that_no_longer_applies_is_dropped_with_the_ops_built_on_it() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&upd("k", |p| p.x = Some(1.0))).unwrap();
    assert!(r.undo().unwrap());
    let ops = r.take_outbox();
    assert_eq!(
        ops[1].deps,
        [ops[0].key.clone()],
        "the undo depends on the edit"
    );
    let mut gone = base();
    gone.retain(|n| n.id != "k");
    assert_eq!(r.resync(gone, 3, &[]).unwrap(), 2);
    assert_eq!(r.pending_len(), 0);
    assert!(r.take_outbox().is_empty());
    r.check_consistency().unwrap();
}

#[test]
fn an_op_that_names_a_dropped_op_is_dropped_even_when_it_would_apply() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&upd("k", |p| p.x = Some(1.0))).unwrap();
    let first = r.unconfirmed()[0].key.clone();
    r.apply_dependent(&upd("a", |p| p.y = Some(2.0)), &[first])
        .unwrap();
    let mut gone = base();
    gone.retain(|n| n.id != "k");
    assert_eq!(
        r.resync(gone, 3, &[]).unwrap(),
        2,
        "the server would refuse the second too"
    );
    assert_eq!(r.pending_len(), 0);
}

#[test]
fn a_replayed_undo_keeps_its_key_and_dependencies() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    assert!(r.undo().unwrap());
    let before = r.unconfirmed();
    r.resync(base(), 3, &[]).unwrap();
    let after = r.unconfirmed();
    assert_eq!(after.len(), 2);
    for (b, a) in before.iter().zip(&after) {
        assert_eq!((&a.key, &a.deps, a.kind), (&b.key, &b.deps, b.kind));
    }
    assert_eq!(after[1].deps, [after[0].key.clone()]);
    r.check_consistency().unwrap();
}

#[test]
fn a_bad_snapshot_is_refused_and_the_replica_is_left_as_it_was() {
    let mut r = diverged();
    let (seq, pending, view) = (r.seq(), r.unconfirmed().len(), snap(r.nodes()));
    let mut bad = base();
    bad.push(node("a", "frame", Some("p")));
    assert!(r.resync(bad, 9, &[]).is_err());
    assert_eq!(
        (r.seq(), r.unconfirmed().len(), snap(r.nodes())),
        (seq, pending, view)
    );
}

#[test]
fn a_removal_the_snapshot_already_holds_leaves_nothing_pending_to_replay() {
    let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
    r.apply(&remove("k")).unwrap();
    let ops = r.take_outbox();
    let mut gone = base();
    gone.retain(|n| n.id != "k");
    r.resync(gone, 2, &[ops[0].key.clone()]).unwrap();
    assert_eq!(r.pending_len(), 0);
    assert!(r.nodes().iter().all(|n| n.id != "k"));
}

#[test]
fn an_op_the_snapshot_holds_but_applied_leaves_out_is_dropped_with_its_undo() {
    let (add_n, with_n) = (common::add("n", "rect", Some("root")), {
        let mut v = base();
        v.push(node("n", "rect", Some("root")));
        v
    });
    let run = |applied_all: bool| {
        let mut r = Replica::new(ACTOR, "s1", base(), 0).unwrap();
        r.apply(&add_n).unwrap();
        assert!(r.undo().unwrap());
        let keys: Vec<String> = r.unconfirmed().iter().map(|o| o.key.clone()).collect();
        assert_eq!(keys.len(), 2, "the add and its undo, which depends on it");
        let applied: &[String] = if applied_all { &keys[..1] } else { &[] };
        let dropped = r.resync(with_n.clone(), 4, applied).unwrap();
        (dropped, r.pending_len())
    };
    // Incomplete: the add is replayed on a snapshot that has `n`, fails as a duplicate, and takes the undo.
    assert_eq!(run(false), (2, 0));
    // Complete: the add is done, its dependent undo is replayed and stays pending.
    assert_eq!(run(true), (0, 1));
}
