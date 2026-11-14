//! Loading an old file (nodes without a `pos`) and rebalancing the siblings of one parent.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{add, node, order_under, snap};
use meldframe_history::FileState;
use meldframe_protocol::Command;

fn old_file() -> Vec<meldframe_protocol::Node> {
    vec![
        node("root", "frame", None),
        node("c", "rect", Some("root")),
        node("a", "rect", Some("root")),
        node("b", "rect", Some("root")),
    ]
}

#[test]
fn loading_an_old_file_gives_every_node_a_pos_in_list_order() {
    let s = FileState::from_nodes_unchecked(old_file());
    assert!(s.nodes().iter().all(|n| !n.pos.is_empty()));
    assert_eq!(order_under(&s, "root"), ["c", "a", "b"]);
    assert_eq!(s.version(), 0);
    assert!(!s.can_undo());
}

#[test]
fn a_file_that_already_has_pos_is_loaded_unchanged() {
    let mut built = FileState::new();
    built.apply(&add("root", "frame", None)).unwrap();
    for id in ["x", "y", "z"] {
        built.apply(&add(id, "rect", Some("root"))).unwrap();
    }
    let loaded = FileState::from_nodes_unchecked(built.nodes().to_vec());
    assert_eq!(snap(&loaded), snap(&built));
}

#[test]
fn a_loaded_old_file_accepts_moves_that_need_a_pos() {
    let mut s = FileState::from_nodes_unchecked(old_file());
    s.apply(&Command::Reorder {
        id: "b".into(),
        before: Some("c".into()),
        after: None,
        pos: None,
    })
    .unwrap();
    assert_eq!(order_under(&s, "root"), ["b", "c", "a"]);
}

#[test]
fn rebalance_keeps_the_order_and_is_one_step_that_undoes_exactly() {
    let mut s = FileState::from_nodes_unchecked(old_file());
    let before = snap(&s);
    let depth = s.undo_depth();
    assert_eq!(s.rebalance(Some("root")), Ok(3));
    assert_eq!(order_under(&s, "root"), ["c", "a", "b"]);
    assert_ne!(snap(&s), before, "every child got a new pos");
    assert_eq!(s.undo_depth(), depth + 1);
    s.undo().unwrap();
    assert_eq!(snap(&s), before);
}

#[test]
fn rebalancing_a_parent_without_children_does_nothing() {
    let mut s = FileState::from_nodes_unchecked(old_file());
    let version = s.version();
    assert_eq!(s.rebalance(Some("c")), Ok(0));
    assert_eq!((s.version(), s.undo_depth()), (version, 0));
}

#[test]
fn rebalance_inside_an_open_group_joins_it_and_does_not_close_it() {
    let mut s = FileState::from_nodes_unchecked(old_file());
    let x = |v: f64| {
        common::upd("a", move |p| {
            p.x = Some(v);
        })
    };
    s.begin_group();
    s.apply(&x(1.0)).unwrap();
    s.rebalance(Some("root")).unwrap();
    s.apply(&x(2.0)).unwrap();
    assert_eq!(s.undo_depth(), 0, "the caller's group is still open");
    s.end_group();
    assert_eq!(s.undo_depth(), 1, "one gesture, one step");
    s.undo().unwrap();
    assert_eq!(s.node("a").unwrap().x, 0.0);
}

#[test]
fn rebalancing_a_parent_without_children_does_not_close_an_open_group() {
    let mut s = FileState::from_nodes_unchecked(old_file());
    s.begin_group();
    s.apply(&common::upd("a", |p| p.x = Some(1.0))).unwrap();
    s.rebalance(Some("c")).unwrap();
    s.apply(&common::upd("a", |p| p.x = Some(2.0))).unwrap();
    s.end_group();
    assert_eq!(s.undo_depth(), 1);
}
