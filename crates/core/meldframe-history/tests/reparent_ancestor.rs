//! Undo and redo of a reparent when the other actor deleted the parent the step would move the node
//! to: the node goes under the nearest ancestor of that parent that survives (the rule an `AddNode`
//! snapshot already follows). If the node itself was deleted, the step is dropped.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "props_common/mod.rs"]
mod common;

use common::{add, snap};
use meldframe_history::FileState;
use meldframe_protocol::{Command, NodePatch};

fn build(nodes: &[(&str, &str, Option<&str>)]) -> FileState {
    let mut s = FileState::new();
    for (id, kind, parent) in nodes {
        s.apply(&add(id, kind, *parent)).unwrap();
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

fn remove(id: &str) -> Command {
    Command::RemoveNode { id: id.into() }
}

fn parent(s: &FileState, id: &str) -> Option<String> {
    s.node(id).unwrap().parent_id.clone()
}

#[test]
fn undo_of_a_move_goes_to_the_surviving_ancestor_and_keeps_my_earlier_edit() {
    // root > p > a, q: I edit root, move a to q, the other actor deletes p.
    let mut s = build(&[
        ("root", "frame", None),
        ("p", "frame", Some("root")),
        ("a", "rect", Some("p")),
        ("q", "frame", Some("root")),
    ]);
    let edit = NodePatch {
        x: Some(5.0),
        ..Default::default()
    };
    s.apply(&Command::UpdateNode {
        id: "root".into(),
        patch: edit,
    })
    .unwrap();
    s.apply(&reparent("a", "q")).unwrap();
    s.apply_remote(&remove("p")).unwrap();
    let undone = s.undo().unwrap();
    assert!(
        matches!(undone.as_slice(), [Command::Reparent { id, parent_id: Some(p), .. }] if id == "a" && p == "root"),
        "{undone:?}"
    );
    assert_eq!(parent(&s, "a").as_deref(), Some("root"));
    assert_eq!(s.node("root").unwrap().x, 5.0, "the earlier edit is kept");
}

#[test]
fn the_nearest_ancestor_is_the_parent_of_the_deleted_subtree_root() {
    // root > outer > mid > p > a; q under root. The other actor deletes mid, and p goes with it.
    let mut s = build(&[
        ("root", "frame", None),
        ("outer", "frame", Some("root")),
        ("mid", "frame", Some("outer")),
        ("p", "frame", Some("mid")),
        ("a", "rect", Some("p")),
        ("q", "frame", Some("root")),
    ]);
    s.apply(&reparent("a", "q")).unwrap();
    s.apply_remote(&remove("mid")).unwrap();
    s.undo().unwrap();
    assert_eq!(parent(&s, "a").as_deref(), Some("outer"));
}

#[test]
fn undo_puts_the_node_at_the_top_when_no_ancestor_of_the_old_parent_is_left() {
    let mut s = build(&[
        ("p", "frame", None),
        ("a", "rect", Some("p")),
        ("q", "frame", None),
    ]);
    s.apply(&reparent("a", "q")).unwrap();
    s.apply_remote(&remove("p")).unwrap();
    assert!(s.undo().is_some());
    assert_eq!(parent(&s, "a"), None);
    assert!(s.node("q").is_some());
}

#[test]
fn redo_after_the_undo_moves_the_node_into_q_again() {
    let mut s = build(&[
        ("root", "frame", None),
        ("p", "frame", Some("root")),
        ("a", "rect", Some("p")),
        ("q", "frame", Some("root")),
    ]);
    s.apply(&reparent("a", "q")).unwrap();
    s.apply_remote(&remove("p")).unwrap();
    s.undo().unwrap();
    let after_undo = snap(&s);
    assert!(s.redo().is_some());
    assert_eq!(parent(&s, "a").as_deref(), Some("q"));
    s.undo().unwrap();
    assert_eq!(snap(&s), after_undo, "undo and redo are exact inverses");
}

#[test]
fn redo_into_a_parent_deleted_meanwhile_goes_to_its_surviving_ancestor() {
    let mut s = build(&[
        ("root", "frame", None),
        ("p", "frame", Some("root")),
        ("a", "rect", Some("p")),
        ("q", "frame", Some("root")),
    ]);
    s.apply(&reparent("a", "q")).unwrap();
    s.undo().unwrap();
    s.apply_remote(&remove("q")).unwrap();
    assert!(s.redo().is_some());
    assert_eq!(parent(&s, "a").as_deref(), Some("root"));
}

#[test]
fn a_step_is_dropped_when_the_node_goes_with_the_parent() {
    let mut s = build(&[
        ("root", "frame", None),
        ("p", "frame", Some("root")),
        ("a", "rect", Some("p")),
        ("g", "frame", Some("root")),
    ]);
    s.apply(&reparent("a", "g")).unwrap();
    s.apply_remote(&remove("g")).unwrap();
    assert!(s.node("a").is_none());
    let depth = s.undo_depth();
    assert_eq!(depth, 2, "the move, the add of a and the add of g are gone");
}

#[test]
fn nested_ancestors_deleted_in_two_remote_ops_end_at_the_last_survivor() {
    // root > outer > mid > p > a; q under root. The other actor deletes mid (a's old parent p goes with it),
    // then outer: the rewritten step moves on to root.
    let mut s = build(&[
        ("root", "frame", None),
        ("outer", "frame", Some("root")),
        ("mid", "frame", Some("outer")),
        ("p", "frame", Some("mid")),
        ("a", "rect", Some("p")),
        ("q", "frame", Some("root")),
    ]);
    s.apply(&reparent("a", "q")).unwrap();
    s.apply_remote(&remove("mid")).unwrap();
    s.apply_remote(&remove("outer")).unwrap();
    s.undo().unwrap();
    assert_eq!(parent(&s, "a").as_deref(), Some("root"));
}

#[test]
fn a_survivor_that_would_make_a_cycle_never_makes_one_and_never_panics() {
    // The other actor moves p under a and then deletes p: the survivor of p's subtree is a itself, so
    // undoing my move would put a under a. The step is refused and undo does not panic.
    let mut s = build(&[
        ("root", "frame", None),
        ("p", "frame", Some("root")),
        ("a", "frame", Some("p")),
        ("q", "frame", Some("root")),
    ]);
    s.apply(&reparent("a", "q")).unwrap();
    s.apply_remote(&reparent("p", "a")).unwrap();
    s.apply_remote(&remove("p")).unwrap();
    // Undo may fall through to earlier steps (the adds of `build`), but it must not panic or create a cycle.
    let _ = s.undo();
    let mut hops = 0;
    let mut cur = "a".to_string();
    while let Some(p) = s.node(&cur).and_then(|n| n.parent_id.clone()) {
        cur = p;
        hops += 1;
        assert!(hops <= s.nodes().len(), "a cycle in the tree");
        assert_ne!(cur, "a", "a is its own ancestor");
    }
}
