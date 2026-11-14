//! Ops that depend on other ops. An undo or redo is worked out from the optimistic view, so it names
//! the pending ops it was built on as `deps`. The server accepts it only if every dependency was
//! applied; when one is refused, the replica drops its dependents the same way, down the chain.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{ServerRule, add, base, node, remove, snap, upd, x_of};
use meldframe_protocol::{Command, MAX_OP_DEPS};
use meldframe_sync::{EventOutcome, OpKind, Replica};

fn keys(r: &Replica) -> Vec<String> {
    r.unconfirmed().into_iter().map(|o| o.key).collect()
}

fn confirm_all(r: &mut Replica, ops: &[meldframe_sync::Outgoing], from: u64) {
    for (i, o) in ops.iter().enumerate() {
        r.on_event(from + i as u64, "user:a", &o.command).unwrap();
    }
}

#[test]
fn only_undo_and_redo_carry_deps_and_only_on_unconfirmed_ops() {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    a.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    a.apply(&upd("a", |p| p.y = Some(2.0))).unwrap();
    let e = a.take_outbox();
    assert!(
        e.iter().all(|o| o.deps.is_empty()),
        "an edit carries no deps"
    );
    // Two pending edits on different fields: undo of a.y depends only on the edit that wrote a.y.
    assert!(a.undo().unwrap());
    let u = a.take_outbox();
    assert_eq!(u[0].deps, [e[1].key.clone()]);
    confirm_all(&mut a, &e, 1);
    assert!(a.redo().unwrap());
    let r = a.take_outbox();
    assert_eq!(
        r[0].deps,
        [u[0].key.clone()],
        "a confirmed op is not a dependency"
    );
    assert_eq!(r[0].kind, OpKind::Redo);
    a.on_event(3, "user:a", &u[0].command).unwrap();
    a.on_event(4, "user:a", &r[0].command).unwrap();
    assert!(a.undo().unwrap());
    assert!(
        a.take_outbox()[0].deps.is_empty(),
        "nothing pending: no deps"
    );

    let mut b = Replica::new("user:b", "cb", base(), 0);
    b.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    b.undo().unwrap();
    let again = b.unconfirmed();
    assert_eq!(
        again[1].deps,
        [again[0].key.clone()],
        "a repeat carries the same deps"
    );
}

#[test]
fn the_ops_of_one_undo_step_depend_on_each_other() {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    a.begin_group();
    a.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    a.apply(&upd("k", |p| p.x = Some(2.0))).unwrap();
    a.end_group();
    let e = a.take_outbox();
    confirm_all(&mut a, &e, 1);
    assert!(a.undo().unwrap());
    let u = a.take_outbox();
    assert_eq!(u.len(), 2);
    assert!(u[0].deps.is_empty());
    assert_eq!(u[1].deps, [u[0].key.clone()]);
}

#[test]
fn deps_keep_the_newest_max_op_deps() {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    let n = MAX_OP_DEPS + 5;
    for i in 0..n {
        a.apply(&upd("a", |p| p.x = Some(i as f64))).unwrap();
    }
    let e = a.take_outbox();
    assert!(a.undo().unwrap());
    let u = a.take_outbox();
    assert_eq!(u[0].deps.len(), MAX_OP_DEPS);
    assert_eq!(u[0].deps.last(), Some(&e[n - 1].key));
    assert_eq!(u[0].deps.first(), Some(&e[5].key));
}

#[test]
fn a_refused_op_takes_its_pending_dependents_with_it() {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    a.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    a.apply(&upd("k", |p| p.x = Some(7.0))).unwrap();
    assert!(a.undo().unwrap());
    let ops = a.take_outbox();
    assert_eq!(
        ops[2].deps,
        [ops[1].key.clone()],
        "only the op that wrote k.x"
    );
    assert!(a.on_reject(&ops[1].key));
    assert_eq!(
        keys(&a),
        [ops[0].key.clone()],
        "the undo is dropped, the unrelated edit stays"
    );
    assert_eq!((x_of(a.nodes(), "k"), x_of(a.nodes(), "a")), (0.0, 10.0));
    a.check_consistency().unwrap();
    assert!(
        !a.on_reject(&ops[2].key),
        "the server's refusal of the dependent is a no-op"
    );
    a.on_event(1, "user:a", &ops[0].command).unwrap();
    a.check_consistency().unwrap();
}

#[test]
fn an_undo_does_not_depend_on_an_unrelated_pending_op() {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    a.apply(&upd("k", |p| p.x = Some(9.0))).unwrap();
    a.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    assert!(a.undo().unwrap());
    let ops = a.take_outbox();
    assert_eq!(ops[2].deps, [ops[1].key.clone()]);
    // The server refuses the edit of k (the node is gone there) and accepts the undo of a.
    let mut rule = ServerRule::new(base());
    rule.nodes.retain(|n| n.id != "k");
    assert_eq!(rule.submit(&ops[0]), Err("invalid"));
    assert_eq!(rule.submit(&ops[1]), Ok(()));
    assert_eq!(rule.submit(&ops[2]), Ok(()));
    assert!(a.on_reject(&ops[0].key));
    assert_eq!(keys(&a), [ops[1].key.clone(), ops[2].key.clone()]);
    a.check_consistency().unwrap();
}

#[test]
fn deps_are_per_field_of_a_node() {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    a.apply(&upd("k", |p| p.x = Some(9.0))).unwrap();
    a.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    a.apply(&upd("a", |p| p.y = Some(6.0))).unwrap();
    assert!(a.undo().unwrap());
    let ops = a.take_outbox();
    assert_eq!(ops[3].deps, [ops[2].key.clone()], "a.x is not involved");
    assert!(a.on_reject(&ops[2].key));
    assert!(!keys(&a).contains(&ops[3].key));
    assert_eq!(keys(&a).len(), 2);
}

#[test]
fn deps_follow_what_an_op_creates_and_removes() {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    a.apply(&upd("p", |p| p.x = Some(3.0))).unwrap();
    a.apply(&add("n", "rect", Some("a"))).unwrap();
    a.apply(&upd("n", |p| p.fill = Some("#ff0000".into())))
        .unwrap();
    assert!(a.undo().unwrap());
    let u = a.take_outbox();
    assert_eq!(
        u[3].deps,
        [u[1].key.clone(), u[2].key.clone()],
        "the add and the fill, not the edit of p"
    );
    a.on_event(1, "user:a", &u[0].command).unwrap();
    a.apply(&upd("root", |p| p.x = Some(1.0))).unwrap();
    a.apply(&remove("a")).unwrap();
    assert!(a.undo().unwrap());
    let r = a.take_outbox();
    let (unrelated, removal) = (r[0].key.clone(), r[1].key.clone());
    assert!(
        r.len() > 3,
        "the undo of a subtree delete adds several nodes"
    );
    assert!(r.iter().skip(2).all(|o| o.deps.contains(&removal)));
    assert!(r.iter().skip(2).all(|o| !o.deps.contains(&unrelated)));
    assert!(
        r.iter().skip(2).any(|o| o.deps.contains(&u[2].key)),
        "the fill of n shapes its snapshot"
    );
}

#[test]
fn deps_are_transitive_through_pending_undo_ops() {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    a.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    a.apply(&upd("a", |p| p.x = Some(2.0))).unwrap();
    assert!(a.undo().unwrap());
    assert!(a.undo().unwrap());
    let o = a.take_outbox();
    assert_eq!(o[2].deps, [o[0].key.clone(), o[1].key.clone()]);
    assert!(o[3].deps.contains(&o[2].key));
    assert!(a.on_reject(&o[0].key));
    assert_eq!(
        keys(&a),
        [o[1].key.clone()],
        "both undos fall with the first edit"
    );
}

#[test]
fn an_explicit_dependency_names_only_ops_that_are_still_pending() {
    let mut a = Replica::new("user:a", "ca", base(), 0);
    a.apply(&upd("a", |p| p.x = Some(5.0))).unwrap();
    let e = a.take_outbox();
    a.apply_dependent(
        &upd("a", |p| p.x = Some(0.0)),
        &[e[0].key.clone(), "old-key".into()],
    )
    .unwrap();
    let d = a.take_outbox();
    assert_eq!(d[0].deps, [e[0].key.clone()]);
    assert_eq!(d[0].kind, OpKind::Edit);
    assert!(a.on_reject(&e[0].key));
    assert_eq!(a.pending_len(), 0);
    assert_eq!(x_of(a.nodes(), "a"), 0.0);
    a.check_consistency().unwrap();
}

#[test]
fn an_undo_built_on_ops_the_server_refuses_is_refused_too_and_the_replica_agrees() {
    // u2 edits above f1 (pending) and undoes; u1 (concurrent) deletes f1. In server order: u1's delete,
    // then u2's pending ops (refused: f1 is gone), then u2's undo, which names them and is refused.
    let init = vec![
        node("root", "frame", None),
        node("f1", "frame", Some("root")),
        node("f2", "frame", Some("root")),
        node("s1", "rect", Some("f1")),
    ];
    let mut rule = ServerRule::new(init.clone());
    let mut u1 = Replica::new("user:u1", "c1", init.clone(), 0);
    let mut u2 = Replica::new("user:u2", "c2", init, 0);
    let first = Command::Reorder {
        id: "f2".into(),
        before: Some("f1".into()),
        after: None,
        pos: None,
    };
    u1.apply(&first).unwrap();
    let o = u1.take_outbox().remove(0);
    rule.submit(&o).unwrap();
    u1.on_event(1, "user:u1", &o.command).unwrap();
    u2.on_event(1, "user:u1", &o.command).unwrap();
    let u1_pos = rule
        .nodes
        .iter()
        .find(|n| n.id == "f2")
        .unwrap()
        .pos
        .clone();

    u2.apply(&Command::Reparent {
        id: "f2".into(),
        parent_id: Some("f1".into()),
        pos: Some("a0V".into()),
    })
    .unwrap();
    u2.begin_group();
    u2.apply(&Command::Reorder {
        id: "f2".into(),
        before: None,
        after: Some("s1".into()),
        pos: None,
    })
    .unwrap();
    u2.end_group();
    assert!(u2.undo().unwrap());
    let ops = u2.take_outbox();
    assert_eq!(ops.len(), 3);
    assert!(ops[0].deps.is_empty() && ops[1].deps.is_empty());
    assert_eq!(ops[2].deps, ["c2-1", "c2-2"]);
    assert!(matches!(&ops[2].command, Command::Reorder { pos: Some(p), .. } if p == "a0V"));

    let rm = remove("f1");
    rule.submit(&meldframe_sync::Outgoing {
        key: "u1-rm".into(),
        command: rm.clone(),
        kind: OpKind::Edit,
        seen_seq: 1,
        deps: vec![],
    })
    .unwrap();
    assert_eq!(rule.submit(&ops[0]), Err("invalid"));
    assert_eq!(rule.submit(&ops[1]), Err("invalid"));
    assert_eq!(rule.submit(&ops[2]), Err("dependency_rejected"));
    assert_eq!(
        rule.nodes.iter().find(|n| n.id == "f2").unwrap().pos,
        u1_pos
    );

    // The replica drops all three when the delete arrives, the undo through the dependency.
    assert_eq!(
        u2.on_event(2, "user:u1", &rm),
        Ok(EventOutcome::Rebased { dropped: 3 })
    );
    assert_eq!(u2.pending_len(), 0);
    assert!(u2.unconfirmed().is_empty(), "the undo is not sent again");
    assert!(!u2.on_reject("c2-1") && !u2.on_reject("c2-2") && !u2.on_reject("c2-3"));
    assert_eq!(snap(u2.nodes()), snap(&rule.nodes));
}
