//! Undo and redo of one actor: the inverse of each kind of command, and what a new edit does to redo.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{add, base, order_under, snap, upd};
use meldframe_history::FileState;
use meldframe_protocol::{Command, ImageFill};

fn with_three_children() -> FileState {
    let mut s = base();
    for id in ["s1", "s2", "s3"] {
        s.apply(&add(id, "rect", Some("root"))).unwrap();
    }
    s
}

#[test]
fn undo_restores_only_the_fields_the_update_touched() {
    let mut s = base();
    s.apply(&upd("a", |p| p.fill = Some("#ff0000".into())))
        .unwrap();
    let before = snap(&s);
    s.apply(&upd("a", |p| {
        p.x = Some(50.0);
        p.w = Some(10.0);
    }))
    .unwrap();
    assert!(s.undo().is_some());
    assert_eq!(snap(&s), before);
    assert!(s.redo().is_some());
    let a = s.node("a").unwrap();
    assert_eq!(
        (a.x, a.w, a.fill.lit().unwrap().as_str()),
        (50.0, 10.0, "#ff0000")
    );
}

#[test]
fn undo_of_an_image_patch_removes_the_image_again() {
    let mut s = base();
    let before = snap(&s);
    let fill: ImageFill =
        serde_json::from_value(serde_json::json!({"assetId": "a".repeat(64)})).unwrap();
    s.apply(&upd("k", |p| p.image = Some(Some(fill)))).unwrap();
    assert!(s.node("k").unwrap().image.is_some());
    s.undo().unwrap();
    assert!(s.node("k").unwrap().image.is_none());
    assert_eq!(snap(&s), before);
}

#[test]
fn undo_of_add_removes_the_node_and_redo_puts_it_back_at_the_same_pos() {
    let mut s = with_three_children();
    let with_all = snap(&s);
    s.apply(&add("s4", "rect", Some("root"))).unwrap();
    let with_four = snap(&s);
    let applied = s.undo().expect("a step to undo");
    assert!(matches!(applied.as_slice(), [Command::RemoveNode { id }] if id == "s4"));
    assert_eq!(snap(&s), with_all);
    s.redo().unwrap();
    assert_eq!(snap(&s), with_four);
}

#[test]
fn undo_of_reorder_and_reparent_restores_parent_and_pos() {
    let mut s = with_three_children();
    let start = snap(&s);
    s.apply(&Command::Reorder {
        id: "s3".into(),
        before: Some("s1".into()),
        after: None,
        pos: None,
    })
    .unwrap();
    assert_eq!(order_under(&s, "root"), ["p", "s3", "s1", "s2"]);
    s.apply(&Command::Reparent {
        id: "s2".into(),
        parent_id: Some("p".into()),
        pos: None,
    })
    .unwrap();
    s.undo().unwrap();
    s.undo().unwrap();
    assert_eq!(snap(&s), start);
    assert_eq!(order_under(&s, "root"), ["p", "s1", "s2", "s3"]);
}

#[test]
fn undo_of_remove_restores_the_subtree_and_the_sibling_order() {
    let mut s = with_three_children();
    s.apply(&add("c", "rect", Some("p"))).unwrap();
    let start = snap(&s);
    s.apply(&Command::RemoveNode { id: "p".into() }).unwrap();
    assert!(s.node("a").is_none() && s.node("k").is_none() && s.node("c").is_none());
    let restored = s.undo().unwrap();
    assert_eq!(restored.len(), 4, "p, a, k and c come back as four adds");
    assert_eq!(snap(&s), start);
    assert_eq!(order_under(&s, "root"), ["p", "s1", "s2", "s3"]);
    s.redo().unwrap();
    assert!(s.node("p").is_none());
}

#[test]
fn a_new_edit_discards_redo() {
    let mut s = base();
    s.apply(&upd("a", |p| p.x = Some(1.0))).unwrap();
    s.undo().unwrap();
    assert!(s.can_redo());
    s.apply(&upd("a", |p| p.x = Some(2.0))).unwrap();
    assert!(!s.can_redo());
    assert!(s.redo().is_none());
}

#[test]
fn a_refused_command_leaves_no_history() {
    let mut s = FileState::from_nodes_unchecked(vec![]);
    assert!(s.apply(&Command::RemoveNode { id: "nope".into() }).is_err());
    assert!(!s.can_undo() && !s.can_redo());
    assert!(s.undo().is_none() && s.redo().is_none());
    assert_eq!(s.version(), 0);
}

#[test]
fn undo_and_redo_each_bump_the_version() {
    let mut s = FileState::new();
    s.apply(&add("a", "rect", None)).unwrap();
    s.undo().unwrap();
    assert_eq!(s.version(), 2);
    s.redo().unwrap();
    assert_eq!(s.version(), 3);
    assert_eq!((s.undo_depth(), s.redo_depth()), (1, 0));
}

#[test]
fn history_keeps_the_newest_five_hundred_steps() {
    let mut s = FileState::new();
    s.apply(&add("a", "rect", None)).unwrap();
    for i in 0..500 {
        s.apply(&upd("a", |p| p.x = Some(f64::from(i)))).unwrap();
    }
    assert_eq!(s.undo_depth(), 500);
    let mut undone = 0;
    while s.undo().is_some() {
        undone += 1;
    }
    assert_eq!(undone, 500);
    assert!(
        s.node("a").is_some(),
        "the oldest step (the add) was dropped"
    );
}

#[test]
fn undo_of_remove_works_when_a_child_is_listed_before_its_parent() {
    // Reparent keeps the list position, so after moving `a` under the later `late` the list order is
    // `a, late` while the tree order is `late, a`. The adds that undo the remove must go parent first.
    let mut s = FileState::new();
    s.apply(&add("a", "frame", None)).unwrap();
    s.apply(&add("late", "frame", None)).unwrap();
    s.apply(&Command::Reparent {
        id: "a".into(),
        parent_id: Some("late".into()),
        pos: None,
    })
    .unwrap();
    let start = snap(&s);
    s.apply(&Command::RemoveNode { id: "late".into() }).unwrap();
    assert!(s.nodes().is_empty());
    let restored = s.undo().unwrap();
    assert_eq!(restored.len(), 2);
    assert_eq!(snap(&s), start);
}
