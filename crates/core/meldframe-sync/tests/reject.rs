//! The server refuses one of my ops: the replica takes its result out of the view, applies the ops
//! that came after it again and forgets its history step.

mod common;

use common::{add, base, snap, upd, x_of};
use meldframe_sync::Replica;

fn keys(r: &Replica) -> Vec<String> {
    r.unconfirmed().into_iter().map(|o| o.key).collect()
}

#[test]
fn a_refused_op_leaves_the_view_and_the_history() {
    let mut r = Replica::new("user:a", "ca", base(), 0);
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    let ops = r.take_outbox();
    assert!(r.on_reject(&ops[0].key));
    assert_eq!(x_of(r.nodes(), "a"), 0.0);
    assert_eq!(r.pending_len(), 0);
    assert!(
        !r.can_undo(),
        "an edit that never took effect leaves no step"
    );
    assert_eq!(snap(r.nodes()), snap(r.base()));
    r.check_consistency().unwrap();
}

#[test]
fn a_refusal_is_ignored_when_the_op_is_not_pending() {
    let mut r = Replica::new("user:a", "ca", base(), 0);
    assert!(!r.on_reject("ca-9"));
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    let ops = r.take_outbox();
    assert!(r.on_reject(&ops[0].key));
    assert!(
        !r.on_reject(&ops[0].key),
        "the second refusal of the same key does nothing"
    );
}

#[test]
fn ops_after_a_refused_one_are_applied_again_on_the_new_state() {
    let mut r = Replica::new("user:a", "ca", base(), 0);
    r.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    r.apply(&upd("a", |p| p.x = Some(9.0))).unwrap();
    r.apply(&upd("k", |p| p.x = Some(1.0))).unwrap();
    let ops = r.take_outbox();
    assert!(r.on_reject(&ops[0].key));
    assert_eq!(keys(&r), [ops[1].key.clone(), ops[2].key.clone()]);
    assert_eq!((x_of(r.nodes(), "a"), x_of(r.nodes(), "k")), (9.0, 1.0));
    r.check_consistency().unwrap();
    for (i, o) in ops[1..].iter().enumerate() {
        r.on_event(i as u64 + 1, "user:a", &o.command).unwrap();
    }
    // The undo of the second edit goes back to what the server had, not to the refused 5.
    assert!(r.undo().unwrap() && r.undo().unwrap());
    assert_eq!((x_of(r.nodes(), "a"), x_of(r.nodes(), "k")), (0.0, 0.0));
}

#[test]
fn an_op_that_needs_a_refused_op_is_dropped_with_it() {
    let mut r = Replica::new("user:a", "ca", base(), 0);
    r.apply(&add("n", "rect", Some("a"))).unwrap();
    r.apply(&upd("n", |p| p.x = Some(4.0))).unwrap();
    r.apply(&upd("k", |p| p.x = Some(2.0))).unwrap();
    let ops = r.take_outbox();
    assert!(r.on_reject(&ops[0].key));
    assert_eq!(
        keys(&r),
        [ops[2].key.clone()],
        "the update of n has nothing to update"
    );
    assert!(r.nodes().iter().all(|n| n.id != "n"));
    assert_eq!(x_of(r.nodes(), "k"), 2.0);
    r.check_consistency().unwrap();
}
