//! Undo against the current state: my undo never overwrites what another actor changed after me.
//! `apply_remote` applies another actor's command (already put in order by the server) without recording
//! it in my history, and rewrites my history so it stays valid.

mod common;

use common::{add, base, upd};
use meldframe_history::{ApplyError, FileState};
use meldframe_protocol::Command;

fn remove(id: &str) -> Command {
    Command::RemoveNode { id: id.into() }
}

fn parent(s: &FileState, id: &str) -> Option<String> {
    s.node(id).unwrap().parent_id.clone()
}

#[test]
fn undo_keeps_the_other_actors_fields_and_restores_my_own() {
    let mut s = base();
    s.apply(&upd("a", |p| {
        p.x = Some(50.0);
        p.y = Some(60.0);
    }))
    .unwrap();
    // The other actor writes fill and y of the same node after me.
    s.apply_remote(&upd("a", |p| {
        p.fill = Some("#ff0000".into());
        p.y = Some(999.0);
    }))
    .unwrap();
    assert!(s.undo().is_some());
    let a = s.node("a").unwrap();
    assert_eq!(a.x, 0.0, "my x is restored");
    assert_eq!(a.y, 999.0, "y was written after me: not overwritten");
    assert_eq!(a.fill, "#ff0000");
    assert!(s.redo().is_some());
    let a = s.node("a").unwrap();
    assert_eq!((a.x, a.y, a.fill.as_str()), (50.0, 999.0, "#ff0000"));
}

#[test]
fn redo_does_not_overwrite_a_field_written_by_the_other_actor_either() {
    let mut s = base();
    s.apply(&upd("a", |p| {
        p.x = Some(5.0);
        p.y = Some(6.0);
    }))
    .unwrap();
    s.undo().unwrap();
    s.apply_remote(&upd("a", |p| p.y = Some(77.0))).unwrap();
    s.redo().unwrap();
    let a = s.node("a").unwrap();
    assert_eq!((a.x, a.y), (5.0, 77.0));
}

#[test]
fn a_step_that_is_fully_overwritten_is_skipped() {
    let mut s = base();
    s.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    s.apply(&upd("p", |p| p.x = Some(7.0))).unwrap();
    s.apply_remote(&upd("p", |p| p.x = Some(8.0))).unwrap();
    // The top step (p.x) has nothing left to restore, so one undo goes to the step below it.
    assert!(s.undo().is_some());
    assert_eq!(s.node("p").unwrap().x, 8.0);
    assert_eq!(s.node("a").unwrap().x, 0.0);
    assert!(s.undo().is_some(), "the adds below are still there");
}

#[test]
fn undo_of_a_delete_reattaches_to_the_nearest_surviving_ancestor() {
    let mut s = base();
    s.apply(&remove("a")).unwrap(); // removes a and k
    s.apply_remote(&remove("p")).unwrap(); // the other actor removes the parent
    assert!(s.undo().is_some());
    assert_eq!(parent(&s, "a").as_deref(), Some("root"));
    assert_eq!(parent(&s, "k").as_deref(), Some("a"));
    assert!(s.node("p").is_none(), "their delete is not undone");
}

#[test]
fn undo_of_an_update_on_a_node_the_other_actor_deleted_is_dropped() {
    let mut s = base();
    let depth = s.undo_depth();
    s.apply(&upd("k", |p| p.x = Some(5.0))).unwrap();
    s.apply_remote(&remove("a")).unwrap();
    assert_eq!(
        s.undo_depth(),
        depth - 2,
        "the adds of a and k and the update of k are gone"
    );
    assert!(s.node("k").is_none());
}

#[test]
fn undo_of_a_delete_brings_back_the_values_the_other_actor_wrote_meanwhile() {
    let mut s = base();
    s.apply(&remove("a")).unwrap();
    s.undo().unwrap();
    s.apply_remote(&upd("k", |p| p.fill = Some("#00ff00".into())))
        .unwrap();
    s.redo().unwrap();
    assert!(s.node("k").is_none());
    s.undo().unwrap();
    assert_eq!(
        s.node("k").unwrap().fill,
        "#00ff00",
        "the value then, not the old one"
    );
}

#[test]
fn a_redo_of_a_delete_is_dropped_when_the_other_actor_deleted_an_ancestor() {
    let mut s = base();
    s.apply(&remove("a")).unwrap();
    s.undo().unwrap();
    s.apply_remote(&remove("p")).unwrap();
    assert!(s.redo().is_none());
    assert!(s.node("a").is_none());
}

#[test]
fn steps_of_an_old_incarnation_are_dropped_when_the_id_is_created_again() {
    let mut s = base();
    s.apply(&upd("k", |p| p.x = Some(5.0))).unwrap();
    s.apply(&remove("k")).unwrap();
    s.apply_remote(&add("k", "rect", Some("a"))).unwrap();
    // Everything I did to the old `k` (its add, update and remove) is gone from my history.
    assert_eq!(s.undo_depth(), 3, "only the adds of root, p and a are left");
    assert!(
        s.node("k").is_some(),
        "their new k is not touched by my history"
    );
}

#[test]
fn a_remote_command_is_not_recorded_and_a_refused_one_changes_nothing() {
    let mut s = base();
    s.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    let (depth, version) = (s.undo_depth(), s.version());
    s.apply_remote(&upd("a", |p| p.w = Some(9.0))).unwrap();
    assert_eq!((s.undo_depth(), s.version()), (depth, version + 1));
    assert_eq!(
        s.apply_remote(&remove("nope")),
        Err(ApplyError::NotFound("nope".into()))
    );
    assert_eq!((s.undo_depth(), s.version()), (depth, version + 1));
    let mut only_remote = FileState::new();
    only_remote.apply_remote(&add("a", "rect", None)).unwrap();
    assert!(!only_remote.can_undo());
}

#[test]
fn a_keyed_edit_does_not_merge_into_an_older_step_when_a_remote_edit_emptied_the_top_one() {
    let mut s = base();
    s.apply(&upd("a", |p| p.y = Some(7.0))).unwrap();
    s.apply_keyed(&upd("a", |p| p.x = Some(1.0)), Some("K"))
        .unwrap();
    // The other actor overwrites the only field my top step changed: that step is pruned.
    s.apply_remote(&upd("a", |p| p.x = Some(50.0))).unwrap();
    let depth = s.undo_depth();
    s.apply_keyed(&upd("a", |p| p.w = Some(9.0)), Some("K"))
        .unwrap();
    assert_eq!(
        s.undo_depth(),
        depth + 1,
        "a remote edit closes the merge run"
    );
    s.undo().unwrap();
    assert_eq!(
        s.node("a").unwrap().y,
        7.0,
        "my older edit is still in place"
    );
}
