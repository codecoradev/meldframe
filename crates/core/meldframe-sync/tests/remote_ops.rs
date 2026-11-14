//! Another actor's ops reach a `Replica` in the server's order. The confirmed state moves on, my
//! pending ops are applied again on top of it, and my undo history is rewritten so that undo and
//! redo never overwrite what the other actor did.

mod common;

use common::{add, base, node, remove, snap, upd, x_of};
use meldframe_order::sib_cmp;
use meldframe_protocol::{Command, Node};
use meldframe_sync::{EventOutcome, Replica, ReplicaError};

fn parent_of(r: &Replica, id: &str) -> Option<String> {
    r.nodes()
        .iter()
        .find(|n| n.id == id)
        .unwrap()
        .parent_id
        .clone()
}

fn order_under(r: &Replica, parent: &str) -> Vec<String> {
    let mut v: Vec<&Node> = r
        .nodes()
        .iter()
        .filter(|n| n.parent_id.as_deref() == Some(parent))
        .collect();
    v.sort_by(|a, b| sib_cmp(a, b));
    v.into_iter().map(|n| n.id.clone()).collect()
}

#[test]
fn another_actors_op_moves_the_confirmed_state_and_the_view() {
    let mut r = Replica::new("user:a", "ca", base(), 0);
    let out = r.on_event(1, "user:b", &upd("a", |p| p.x = Some(7.0)));
    assert_eq!(out, Ok(EventOutcome::Rebased { dropped: 0 }));
    assert_eq!(
        (r.seq(), x_of(r.base(), "a"), x_of(r.nodes(), "a")),
        (1, 7.0, 7.0)
    );
    assert!(!r.can_undo(), "another actor's op is not in my history");
    r.check_consistency().unwrap();
}

#[test]
fn a_pending_op_is_applied_again_on_top_of_an_earlier_remote_op() {
    // I send x=10. The server orders the other actor's x=5 (and a fill) before it.
    let mut r = Replica::new("user:a", "ca", base(), 0);
    r.apply(&upd("a", |p| p.x = Some(10.0))).unwrap();
    let mine = r.take_outbox();
    let theirs = upd("a", |p| {
        p.x = Some(5.0);
        p.fill = Some("#111111".into());
    });
    assert_eq!(
        r.on_event(1, "user:b", &theirs),
        Ok(EventOutcome::Rebased { dropped: 0 })
    );
    assert_eq!(x_of(r.nodes(), "a"), 10.0, "my pending op wins in the view");
    assert_eq!(r.pending_len(), 1);
    r.check_consistency().unwrap();
    assert_eq!(
        r.on_event(2, "user:a", &mine[0].command),
        Ok(EventOutcome::Confirmed)
    );
    // Undo goes back to what the server had before my op (5), not to what I saw (0).
    assert!(r.undo().unwrap());
    let a = r.nodes().iter().find(|n| n.id == "a").unwrap();
    assert_eq!((a.x, a.fill.as_str()), (5.0, "#111111"));
}

#[test]
fn undo_of_a_delete_brings_back_what_the_other_actor_added_before_it() {
    let mut r = Replica::new("user:a", "ca", base(), 0);
    r.apply(&remove("a")).unwrap();
    let mine = r.take_outbox();
    r.on_event(1, "user:b", &add("kk", "rect", Some("a")))
        .unwrap();
    r.on_event(2, "user:b", &upd("k", |p| p.fill = Some("#abcdef".into())))
        .unwrap();
    assert!(
        r.nodes()
            .iter()
            .all(|n| !["a", "k", "kk"].contains(&n.id.as_str()))
    );
    r.check_consistency().unwrap();
    r.on_event(3, "user:a", &mine[0].command).unwrap();
    assert!(r.undo().unwrap());
    assert_eq!(
        r.nodes().iter().find(|n| n.id == "k").unwrap().fill,
        "#abcdef"
    );
    assert!(
        r.nodes().iter().any(|n| n.id == "kk"),
        "the server's delete took kk too"
    );
}

#[test]
fn a_pending_op_that_is_no_longer_valid_is_dropped_with_its_history() {
    let mut r = Replica::new("user:a", "ca", base(), 0);
    r.apply(&upd("k", |p| p.x = Some(3.0))).unwrap();
    r.take_outbox();
    assert_eq!(
        r.on_event(1, "user:b", &remove("a")),
        Ok(EventOutcome::Rebased { dropped: 1 })
    );
    assert_eq!(r.pending_len(), 0);
    assert!(r.unconfirmed().is_empty(), "a dropped op is not sent again");
    assert!(
        !r.can_undo(),
        "an edit that never took effect leaves no step"
    );
    r.check_consistency().unwrap();
}

#[test]
fn an_event_that_cannot_be_applied_changes_nothing() {
    let mut r = Replica::new("user:a", "ca", base(), 0);
    r.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    let before = (snap(r.nodes()), snap(r.base()), r.seq());
    let e = r.on_event(1, "user:b", &remove("missing"));
    assert!(matches!(e, Err(ReplicaError::Diverged(_))), "{e:?}");
    assert_eq!((snap(r.nodes()), snap(r.base()), r.seq()), before);
}

#[test]
fn my_confirmed_undo_keeps_the_field_the_other_actor_wrote_after_me() {
    let mut r = Replica::new("user:a", "ca", base(), 0);
    r.apply(&upd("a", |p| {
        p.x = Some(50.0);
        p.y = Some(60.0);
    }))
    .unwrap();
    let mine = r.take_outbox();
    r.on_event(1, "user:a", &mine[0].command).unwrap();
    r.on_event(2, "user:b", &upd("a", |p| p.y = Some(999.0)))
        .unwrap();
    assert!(r.undo().unwrap());
    let a = r.nodes().iter().find(|n| n.id == "a").unwrap();
    assert_eq!(
        (a.x, a.y),
        (0.0, 999.0),
        "y was written after me: not overwritten"
    );
    let undo = r.take_outbox();
    assert!(matches!(&undo[0].command, Command::UpdateNode { patch, .. } if patch.y.is_none()));
}

#[test]
fn a_remote_update_reaches_a_node_my_undo_will_add_again() {
    // I add n, then undo (a delete, pending). The other actor's update of n is ordered before my delete.
    let mut r = Replica::new("user:a", "ca", base(), 0);
    r.apply(&add("n", "rect", Some("a"))).unwrap();
    let first = r.take_outbox();
    r.on_event(1, "user:a", &first[0].command).unwrap();
    assert!(r.undo().unwrap());
    let undo = r.take_outbox();
    r.on_event(2, "user:b", &upd("n", |p| p.x = Some(5.0)))
        .unwrap();
    r.on_event(3, "user:a", &undo[0].command).unwrap();
    assert!(r.nodes().iter().all(|n| n.id != "n"));
    assert!(r.redo().unwrap());
    assert_eq!(
        x_of(r.nodes(), "n"),
        5.0,
        "the node comes back with the other actor's edit"
    );
    r.check_consistency().unwrap();
}

#[test]
fn a_remote_reparent_reaches_a_node_my_undo_will_add_again() {
    let mut r = Replica::new("user:a", "ca", base(), 0);
    r.apply(&add("n", "rect", Some("a"))).unwrap();
    let first = r.take_outbox();
    r.on_event(1, "user:a", &first[0].command).unwrap();
    assert!(r.undo().unwrap());
    let undo = r.take_outbox();
    let moved = Command::Reparent {
        id: "n".into(),
        parent_id: Some("p".into()),
        pos: None,
    };
    r.on_event(2, "user:b", &moved).unwrap();
    r.on_event(3, "user:a", &undo[0].command).unwrap();
    assert!(r.redo().unwrap());
    assert_eq!(parent_of(&r, "n").as_deref(), Some("p"));
}

fn f1_nodes() -> Vec<Node> {
    vec![
        node("root", "frame", None),
        node("f1", "frame", Some("root")),
        node("s1", "rect", Some("f1")),
        node("s2", "rect", Some("f1")),
        node("s4", "text", Some("root")),
    ]
}

#[test]
fn redo_of_a_reparent_survives_a_remote_reorder_of_the_same_node() {
    // u1 reparents s4 into f1 and undoes it. u2's reorder of s4 (ordered first) writes only `pos`,
    // so u1's redo must still move s4 into f1.
    let mut u1 = Replica::new("user:u1", "c1", f1_nodes(), 0);
    let mut u2 = Replica::new("user:u2", "c2", f1_nodes(), 0);
    let reparent = Command::Reparent {
        id: "s4".into(),
        parent_id: Some("f1".into()),
        pos: Some("a0".into()),
    };
    u1.apply(&reparent).unwrap();
    assert!(u1.undo().unwrap());
    let sent = u1.take_outbox();
    let reorder = Command::Reorder {
        id: "s4".into(),
        before: None,
        after: None,
        pos: Some("a0t".into()),
    };
    u2.apply(&reorder).unwrap();
    u2.take_outbox();
    // Server order: u2's reorder (1), then u1's two ops (2, 3).
    u1.on_event(1, "user:u2", &reorder).unwrap();
    u1.on_event(2, "user:u1", &sent[0].command).unwrap();
    u1.on_event(3, "user:u1", &sent[1].command).unwrap();
    u2.on_event(1, "user:u2", &reorder).unwrap();
    u2.on_event(2, "user:u1", &sent[0].command).unwrap();
    u2.on_event(3, "user:u1", &sent[1].command).unwrap();
    assert!(
        u1.can_redo(),
        "a reorder by someone else must not erase my redo"
    );
    assert!(u1.redo().unwrap());
    assert_eq!(parent_of(&u1, "s4").as_deref(), Some("f1"));
    // My history's `pos` (a0, among the children of f1) was not touched by a reorder among the children
    // of root, so redo replays my command as it was: a0 collides with s1 and goes right behind it.
    assert_eq!(order_under(&u1, "f1"), ["s1", "s4", "s2"]);
    let redo = u1.take_outbox();
    u1.on_event(4, "user:u1", &redo[0].command).unwrap();
    u2.on_event(4, "user:u1", &redo[0].command).unwrap();
    assert_eq!(snap(u1.base()), snap(u2.base()));
    u1.check_consistency().unwrap();
    u2.check_consistency().unwrap();
}
