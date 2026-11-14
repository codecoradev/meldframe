//! The server refuses an undo or redo op: the history goes back to where it was, so the step can be
//! tried again. Nothing is left on the wrong stack and an undo is never stuck on a step the server
//! always refuses.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{base, upd, x_of};
use meldframe_sync::Replica;

/// One edit `a.x = 5` that the server confirmed (seq 1).
fn confirmed_edit() -> Replica {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    a.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    let e = a.take_outbox();
    a.on_event(1, "user:a", &e[0].command).unwrap();
    a
}

fn depths(r: &Replica) -> (usize, usize) {
    (r.undo_depth(), r.redo_depth())
}

fn confirm(r: &mut Replica, from: u64) -> u64 {
    let ops = r.take_outbox();
    for (i, o) in ops.iter().enumerate() {
        r.on_event(from + i as u64, "user:a", &o.command).unwrap();
    }
    from + ops.len() as u64
}

#[test]
fn a_refused_undo_puts_the_step_back_and_undo_can_be_tried_again() {
    let mut a = confirmed_edit();
    assert!(a.undo().unwrap());
    let u = a.take_outbox();
    assert!(
        !a.can_undo() && a.can_redo(),
        "while pending the step is on the redo stack"
    );
    assert!(a.on_reject(&u[0].key));
    assert_eq!(depths(&a), (1, 0));
    assert_eq!(x_of(a.nodes(), "a"), 5.0);
    a.check_consistency().unwrap();
    assert!(a.undo().unwrap());
    confirm(&mut a, 2);
    assert_eq!((x_of(a.nodes(), "a"), depths(&a)), (0.0, (0, 1)));
}

#[test]
fn a_refused_redo_puts_the_step_back_on_the_redo_stack() {
    let mut a = confirmed_edit();
    assert!(a.undo().unwrap());
    let next = confirm(&mut a, 2);
    assert!(a.redo().unwrap());
    let r = a.take_outbox();
    assert_eq!(depths(&a), (1, 0));
    assert!(a.on_reject(&r[0].key));
    assert_eq!(depths(&a), (0, 1));
    assert_eq!(x_of(a.nodes(), "a"), 0.0);
    a.check_consistency().unwrap();
    assert!(a.redo().unwrap());
    confirm(&mut a, next);
    assert_eq!(x_of(a.nodes(), "a"), 5.0);
}

#[test]
fn an_undo_that_fell_with_its_refused_edit_leaves_no_history() {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    a.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    assert!(a.undo().unwrap());
    let o = a.take_outbox();
    assert!(a.can_redo());
    assert!(a.on_reject(&o[0].key));
    assert_eq!((a.pending_len(), depths(&a)), (0, (0, 0)));
    assert!(!a.can_undo() && !a.can_redo());
    a.check_consistency().unwrap();
}

#[test]
fn a_refused_undo_goes_back_below_an_edit_made_since() {
    let mut a = confirmed_edit();
    assert!(a.undo().unwrap());
    let u = a.take_outbox();
    a.apply(&upd("k", |p| p.x = Some(7.0))).unwrap();
    a.take_outbox();
    assert_eq!(depths(&a), (1, 0));
    assert!(a.on_reject(&u[0].key));
    assert_eq!(
        depths(&a),
        (2, 0),
        "the first edit is back below the new one"
    );
    assert!(a.undo().unwrap());
    assert_eq!((x_of(a.nodes(), "k"), x_of(a.nodes(), "a")), (0.0, 5.0));
    assert!(a.undo().unwrap());
    assert_eq!((x_of(a.nodes(), "k"), x_of(a.nodes(), "a")), (0.0, 0.0));
}

#[test]
fn a_refused_redo_does_not_come_back_after_a_new_edit_cleared_redo() {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    for f in 0..3 {
        a.apply(&upd("a", |p| match f {
            0 => p.x = Some(1.0),
            1 => p.y = Some(2.0),
            _ => p.w = Some(3.0),
        }))
        .unwrap();
    }
    let mut seq = confirm(&mut a, 1);
    for _ in 0..2 {
        assert!(a.undo().unwrap());
        seq = confirm(&mut a, seq);
    }
    assert_eq!(depths(&a), (1, 2));
    assert!(a.redo().unwrap());
    let r = a.take_outbox();
    a.apply(&upd("k", |p| p.x = Some(7.0))).unwrap();
    a.take_outbox();
    assert_eq!(depths(&a), (3, 0));
    assert!(a.on_reject(&r[0].key));
    assert_eq!(
        depths(&a),
        (2, 0),
        "the failed redo leaves undo, and redo does not come back"
    );
    a.check_consistency().unwrap();
}

#[test]
fn a_step_with_several_ops_comes_back_whole_when_one_op_is_refused() {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    a.begin_group();
    a.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    a.apply(&upd("k", |p| p.x = Some(2.0))).unwrap();
    a.end_group();
    let seq = confirm(&mut a, 1);
    assert!(a.undo().unwrap());
    let u = a.take_outbox();
    assert_eq!(u.len(), 2);
    a.on_event(seq, "user:a", &u[0].command).unwrap();
    assert!(a.on_reject(&u[1].key));
    assert_eq!(
        depths(&a),
        (1, 0),
        "the whole step is back: undo can be tried again"
    );
    a.check_consistency().unwrap();
}

#[test]
fn undo_and_redo_both_pending_with_the_undo_refused_restores_both_stacks() {
    let mut a = confirmed_edit();
    assert!(a.undo().unwrap());
    let u = a.take_outbox();
    assert!(a.redo().unwrap());
    let r = a.take_outbox();
    assert_eq!(r[0].deps, [u[0].key.clone()]);
    assert!(a.on_reject(&u[0].key));
    assert_eq!((a.pending_len(), depths(&a)), (0, (1, 0)));
    assert_eq!(x_of(a.nodes(), "a"), 5.0);
    assert!(a.undo().unwrap(), "the original step can still be undone");
    a.check_consistency().unwrap();
}

#[test]
fn a_step_the_server_refuses_twice_is_dropped_so_undo_is_never_stuck() {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    let mut seq = 1;
    for v in [1.0, 2.0] {
        a.apply(&upd("k", |p| p.x = Some(v))).unwrap();
        seq = confirm(&mut a, seq);
    }
    assert_eq!(a.undo_depth(), 2);
    assert!(a.undo().unwrap());
    let u1 = a.take_outbox();
    assert!(a.on_reject(&u1[0].key));
    assert_eq!(depths(&a), (2, 0), "refused once: put back");
    assert!(a.undo().unwrap());
    let u2 = a.take_outbox();
    assert!(a.on_reject(&u2[0].key));
    assert_eq!(
        depths(&a),
        (1, 0),
        "refused twice: dropped, the step below stays"
    );
    assert!(a.undo().unwrap(), "undo is not stuck");
    let _ = seq;
}
