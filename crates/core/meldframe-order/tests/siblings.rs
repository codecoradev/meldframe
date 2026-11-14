//! Sibling order helpers: `(pos, id)` order, with unpositioned nodes first in list order.

use meldframe_order::{children_map, sib_cmp};
use meldframe_protocol::Node;
use std::cmp::Ordering;

fn node(id: &str, parent: Option<&str>, pos: &str) -> Node {
    serde_json::from_value(
        serde_json::json!({"id": id, "type": "rect", "parentId": parent, "pos": pos}),
    )
    .unwrap()
}

#[test]
fn compares_by_pos_then_id() {
    assert_eq!(
        sib_cmp(&node("b", None, "a0"), &node("a", None, "a1")),
        Ordering::Less
    );
    assert_eq!(
        sib_cmp(&node("a", None, "a1"), &node("b", None, "a1")),
        Ordering::Less
    );
    assert_eq!(
        sib_cmp(&node("a", None, "a1"), &node("a", None, "a1")),
        Ordering::Equal
    );
}

#[test]
fn nodes_without_pos_tie_and_come_first() {
    assert_eq!(
        sib_cmp(&node("b", None, ""), &node("a", None, "")),
        Ordering::Equal
    );
    assert_eq!(
        sib_cmp(&node("z", None, ""), &node("a", None, "a0")),
        Ordering::Less
    );
}

#[test]
fn children_are_grouped_by_parent_and_sorted() {
    let ns = vec![
        node("c", Some("r"), "a2"),
        node("r", None, "a0"),
        node("a", Some("r"), "a0"),
        node("b", Some("r"), "a1"),
    ];
    let kids = children_map(&ns);
    let ids =
        |p: Option<&str>| -> Vec<&str> { kids[&p].iter().map(|&i| ns[i].id.as_str()).collect() };
    assert_eq!(ids(Some("r")), ["a", "b", "c"]);
    assert_eq!(ids(None), ["r"]);
    assert!(!kids.contains_key(&Some("a")));
}

#[test]
fn list_order_does_not_matter_once_positions_exist() {
    let mut ns = vec![node("a", Some("r"), "a0"), node("b", Some("r"), "a1")];
    ns.reverse();
    let kids = children_map(&ns);
    let ids: Vec<&str> = kids[&Some("r")]
        .iter()
        .map(|&i| ns[i].id.as_str())
        .collect();
    assert_eq!(ids, ["a", "b"]);
}

#[test]
fn unpositioned_siblings_keep_list_order() {
    let ns = vec![node("z", Some("r"), ""), node("a", Some("r"), "")];
    let kids = children_map(&ns);
    let ids: Vec<&str> = kids[&Some("r")]
        .iter()
        .map(|&i| ns[i].id.as_str())
        .collect();
    assert_eq!(ids, ["z", "a"]);
}
