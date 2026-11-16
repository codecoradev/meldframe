//! A snapshot is bytes from a network: `Replica::new` refuses a corrupt one with a typed `LoadError` and
//! its stable code, and a good one loads exactly as `FileState::try_from_nodes` would load it.

#[path = "../../meldframe-sync/tests/common/mod.rs"]
mod common;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use common::{base, node};
use meldframe_check::LoadError;
use meldframe_history::{FileState, Limits};
use meldframe_protocol::Node;
use meldframe_sync::Replica;

fn refused(nodes: Vec<Node>) -> LoadError {
    Replica::new("user:a", "ca", nodes, 7).expect_err("the snapshot must be refused")
}

fn json(nodes: &[Node]) -> String {
    serde_json::to_string(nodes).unwrap()
}

fn with_pos(mut n: Node, pos: &str) -> Node {
    n.pos = pos.into();
    n
}

#[test]
fn a_duplicate_id_is_refused() {
    let mut nodes = base();
    nodes.push(node("a", "frame", Some("root")));
    assert_eq!(refused(nodes).code(), "E_DUPLICATE_ID");
}

#[test]
fn a_missing_parent_is_refused() {
    let mut nodes = base();
    nodes.push(node("x", "rect", Some("nowhere")));
    assert_eq!(refused(nodes).code(), "E_PARENT_NOT_FOUND");
}

#[test]
fn a_cycle_is_refused() {
    let nodes = vec![node("a", "frame", Some("b")), node("b", "frame", Some("a"))];
    assert_eq!(refused(nodes).code(), "E_CYCLE");
}

#[test]
fn a_tree_deeper_than_the_limit_is_refused() {
    let max = Limits::default().max_depth;
    let mut nodes = vec![node("n0", "frame", None)];
    for i in 1..=max + 1 {
        nodes.push(node(
            &format!("n{i}"),
            "frame",
            Some(&format!("n{}", i - 1)),
        ));
    }
    assert_eq!(refused(nodes).code(), "E_TOO_DEEP");
}

#[test]
fn a_text_node_with_a_child_is_refused() {
    let mut nodes = base();
    nodes.push(node("x", "rect", Some("k")));
    assert_eq!(refused(nodes).code(), "E_PARENT_NOT_FRAME");
}

#[test]
fn the_load_error_names_the_node() {
    let mut nodes = base();
    nodes.push(node("x", "rect", Some("nowhere")));
    assert!(refused(nodes).to_string().contains("node x"));
}

#[test]
fn a_snapshot_without_pos_migrates_like_try_from_nodes() {
    let r = Replica::new("user:a", "ca", base(), 7).unwrap();
    let want = FileState::try_from_nodes(base(), Limits::default()).unwrap();
    assert_eq!(json(r.nodes()), json(want.nodes()));
    assert_eq!(json(r.base()), json(want.nodes()));
    assert!(r.nodes().iter().all(|n| !n.pos.is_empty()));
    assert_eq!(r.seq(), 7);
    assert_eq!(r.pending_len(), 0);
}

#[test]
fn equal_pos_among_siblings_is_migrated_not_refused() {
    let nodes = vec![
        node("root", "frame", None),
        with_pos(node("a", "rect", Some("root")), "m"),
        with_pos(node("b", "rect", Some("root")), "m"),
    ];
    let r = Replica::new("user:a", "ca", nodes.clone(), 0).unwrap();
    let want = FileState::try_from_nodes(nodes, Limits::default()).unwrap();
    assert_eq!(json(r.nodes()), json(want.nodes()));
    assert_ne!(r.nodes()[1].pos, r.nodes()[2].pos);
}

#[test]
fn a_good_snapshot_with_pos_loads_untouched() {
    let ready = FileState::try_from_nodes(base(), Limits::default()).unwrap();
    let r = Replica::new("user:a", "ca", ready.nodes().to_vec(), 3).unwrap();
    assert_eq!(json(r.nodes()), json(ready.nodes()));
}
