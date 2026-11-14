//! `check_file` is the strict whole-file check: it refuses what `check_command` would never let in, and
//! it never repairs. A missing or repeated `pos` is refused here; the loader migrates it first.

use meldframe_check::{Limits, check_file};
use meldframe_protocol::Node;

fn node(id: &str, kind: &str, parent: Option<&str>, pos: &str) -> Node {
    serde_json::from_value(
        serde_json::json!({"id": id, "type": kind, "parentId": parent, "pos": pos}),
    )
    .unwrap()
}

fn code(nodes: &[Node]) -> &'static str {
    check_file(nodes, &Limits::default()).unwrap_err().code()
}

#[test]
fn a_file_with_unique_valid_positions_passes() {
    let v = vec![
        node("root", "frame", None, "a0"),
        node("a", "rect", Some("root"), "a0"),
        node("b", "rect", Some("root"), "a1"),
    ];
    assert!(check_file(&v, &Limits::default()).is_ok());
}

#[test]
fn an_empty_file_passes() {
    assert!(check_file(&[], &Limits::default()).is_ok());
}

#[test]
fn a_node_without_pos_is_refused() {
    let v = vec![
        node("root", "frame", None, "a0"),
        node("a", "rect", Some("root"), ""),
    ];
    assert_eq!(code(&v), "E_MISSING_POS");
}

#[test]
fn two_siblings_with_the_same_pos_are_refused() {
    let v = vec![
        node("root", "frame", None, "a0"),
        node("a", "rect", Some("root"), "a0"),
        node("b", "rect", Some("root"), "a0"),
    ];
    assert_eq!(code(&v), "E_DUPLICATE_POS");
}

#[test]
fn the_same_pos_under_different_parents_is_fine() {
    let v = vec![
        node("root", "frame", None, "a0"),
        node("p", "frame", Some("root"), "a0"),
        node("a", "rect", Some("root"), "a1"),
        node("b", "rect", Some("p"), "a1"),
        node("c", "rect", Some("p"), "a2"),
    ];
    assert!(check_file(&v, &Limits::default()).is_ok());
}
