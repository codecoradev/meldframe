//! Undo against the current state for moves: reparent and reorder are written together with `pos`,
//! which only means something among the siblings of one parent.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{add, order_under};
use meldframe_history::FileState;
use meldframe_protocol::Command;

/// root > [f1 > [s1, s2], s4, s5]
fn tree() -> FileState {
    let mut s = FileState::new();
    for (id, kind, parent) in [
        ("root", "frame", None),
        ("f1", "frame", Some("root")),
        ("s1", "rect", Some("f1")),
        ("s2", "rect", Some("f1")),
        ("s4", "text", Some("root")),
        ("s5", "rect", Some("root")),
    ] {
        s.apply(&add(id, kind, parent)).unwrap();
    }
    s
}

fn reparent(id: &str, parent: &str) -> Command {
    Command::Reparent {
        id: id.into(),
        parent_id: Some(parent.into()),
        pos: None,
    }
}

fn reorder(id: &str, before: Option<&str>, after: Option<&str>) -> Command {
    Command::Reorder {
        id: id.into(),
        before: before.map(String::from),
        after: after.map(String::from),
        pos: None,
    }
}

fn parent(s: &FileState, id: &str) -> Option<String> {
    s.node(id).unwrap().parent_id.clone()
}

#[test]
fn a_reparent_by_the_other_actor_wins_over_my_undo() {
    let mut s = tree();
    s.apply(&reparent("s4", "f1")).unwrap();
    s.apply_remote(&Command::Reparent {
        id: "s4".into(),
        parent_id: Some("root".into()),
        pos: None,
    })
    .unwrap();
    assert_eq!(s.undo_depth(), 6, "my reparent step is gone");
    assert_eq!(parent(&s, "s4").as_deref(), Some("root"));
}

#[test]
fn undo_of_a_reorder_is_dropped_when_the_other_actor_reparents_the_node() {
    let mut s = tree();
    s.apply(&reorder("s4", Some("f1"), None)).unwrap();
    s.apply_remote(&reparent("s4", "f1")).unwrap();
    // The `pos` I would restore was written among the siblings of root; s4 is not there any more.
    let top = s.undo_depth();
    assert_eq!(top, 6, "the reorder step is gone, the six adds remain");
    assert_eq!(parent(&s, "s4").as_deref(), Some("f1"));
}

#[test]
fn undo_of_a_reorder_is_dropped_when_the_other_actor_reorders_the_node() {
    let mut s = tree();
    s.apply(&reorder("s2", Some("s1"), None)).unwrap();
    s.apply_remote(&reorder("s2", None, Some("s1"))).unwrap();
    assert_eq!(s.undo_depth(), 6);
    assert_eq!(order_under(&s, "f1"), ["s1", "s2"], "their position stays");
}

#[test]
fn undo_of_a_reparent_after_the_other_actor_reorders_in_the_new_parent_restores_the_old_place() {
    let mut s = tree();
    s.apply(&reparent("s4", "f1")).unwrap();
    s.apply_remote(&reorder("s4", Some("s1"), None)).unwrap();
    assert_eq!(order_under(&s, "f1"), ["s4", "s1", "s2"]);
    // Their `pos` lives among the siblings of f1; mine among those of root, so mine is kept.
    assert!(s.undo().is_some());
    assert_eq!(parent(&s, "s4").as_deref(), Some("root"));
    assert_eq!(order_under(&s, "root"), ["f1", "s4", "s5"]);
    assert_eq!(order_under(&s, "f1"), ["s1", "s2"]);
}

#[test]
fn a_redo_of_a_reparent_survives_a_remote_reorder_of_the_same_node() {
    let mut s = tree();
    s.apply(&reparent("s4", "f1")).unwrap();
    s.undo().unwrap();
    s.apply_remote(&reorder("s4", Some("f1"), None)).unwrap();
    assert!(
        s.can_redo(),
        "reorder writes no parent: the redo is still valid"
    );
    assert!(s.redo().is_some());
    assert_eq!(parent(&s, "s4").as_deref(), Some("f1"));
}

#[test]
fn a_remote_reorder_drops_my_pos_only_in_the_parent_it_wrote_in() {
    // s4 goes root -> f1 -> root (both mine); the other actor reorders s4 among the siblings of root.
    let mut s = tree();
    s.apply(&reparent("s4", "f1")).unwrap();
    s.apply(&reparent("s4", "root")).unwrap();
    s.apply_remote(&reorder("s4", Some("f1"), None)).unwrap();
    let first = s.undo().unwrap();
    assert!(
        matches!(first.as_slice(), [Command::Reparent { parent_id: Some(p), pos: Some(_), .. }] if p == "f1"),
        "{first:?}"
    );
    let second = s.undo().unwrap();
    assert!(
        matches!(second.as_slice(), [Command::Reparent { parent_id: Some(p), pos: None, .. }] if p == "root"),
        "{second:?}"
    );
}

#[test]
fn a_move_is_dropped_when_the_other_actor_deletes_the_node() {
    let mut s = tree();
    s.apply(&reparent("s2", "root")).unwrap();
    s.apply_remote(&Command::RemoveNode { id: "s2".into() })
        .unwrap();
    assert_eq!(s.undo_depth(), 5, "the move and the add of s2 are gone");
}

#[test]
fn a_move_is_not_undone_into_a_parent_the_other_actor_deleted() {
    let mut s = tree();
    s.apply(&reparent("s2", "root")).unwrap();
    s.apply_remote(&Command::RemoveNode { id: "f1".into() })
        .unwrap();
    // Undoing the move would put s2 back under f1, so the move is skipped and undo takes the step below
    // it, the add of s5. s2 stays where it is.
    let undone = s.undo().unwrap();
    assert!(matches!(undone.as_slice(), [Command::RemoveNode { id }] if id == "s5"));
    assert_eq!(parent(&s, "s2").as_deref(), Some("root"));
}

#[test]
fn moves_of_an_old_incarnation_are_dropped_when_the_id_is_created_again() {
    let mut s = tree();
    s.apply(&reorder("s5", Some("f1"), None)).unwrap();
    s.apply(&reparent("s5", "f1")).unwrap();
    s.apply(&Command::RemoveNode { id: "s5".into() }).unwrap();
    s.apply_remote(&add("s5", "rect", Some("root"))).unwrap();
    assert_eq!(s.undo_depth(), 5, "nothing of the old s5 is left");
    assert!(s.node("s5").is_some());
}

#[test]
fn a_reparent_step_whose_node_was_deleted_is_dropped_and_undo_moves_on() {
    let mut s = tree();
    s.apply(&add("g", "frame", Some("root"))).unwrap();
    s.apply(&common::upd("root", |p| p.x = Some(5.0))).unwrap();
    s.apply(&reparent("s1", "g")).unwrap();
    // s1 went away together with g: nothing of my move can apply, so undo reaches my earlier edit.
    s.apply_remote(&Command::RemoveNode { id: "g".into() })
        .unwrap();
    assert!(s.node("s1").is_none());
    s.undo().unwrap();
    assert_eq!(s.node("root").unwrap().x, 0.0);
}
