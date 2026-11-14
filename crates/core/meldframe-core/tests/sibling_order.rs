//! Sibling order: effective order is `(pos, id)`, never the position in the list. Covers add,
//! reorder, reparent, colliding positions and refused positions.

use meldframe_core::{ApplyError, Limits, apply_command};
use meldframe_order::{children_map, fracindex::between};
use meldframe_protocol::{Command, Node};

fn node(id: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent})).unwrap()
}

fn node_pos(id: &str, parent: Option<&str>, pos: &str) -> Node {
    serde_json::from_value(
        serde_json::json!({"id": id, "type": "rect", "parentId": parent, "pos": pos}),
    )
    .unwrap()
}

fn add(n: Node) -> Command {
    Command::AddNode {
        node: n,
        index: None,
    }
}

fn reorder(id: &str, before: Option<&str>, after: Option<&str>, pos: Option<&str>) -> Command {
    Command::Reorder {
        id: id.into(),
        before: before.map(Into::into),
        after: after.map(Into::into),
        pos: pos.map(Into::into),
    }
}

fn order(nodes: &[Node], parent: Option<&str>) -> Vec<String> {
    let kids = children_map(nodes);
    let ids = kids
        .get(&parent)
        .map(|v| v.iter().map(|&i| nodes[i].id.clone()).collect());
    ids.unwrap_or_default()
}

fn pos_of(nodes: &[Node], id: &str) -> String {
    nodes.iter().find(|n| n.id == id).unwrap().pos.clone()
}

fn apply(nodes: &mut Vec<Node>, cmd: &Command) -> Result<(), ApplyError> {
    apply_command(nodes, cmd, &Limits::default())
}

/// A root frame holding rects `a`, `b`, `c` in that order.
fn base_doc() -> Vec<Node> {
    let mut ns = vec![];
    apply(&mut ns, &add(node("root", "frame", None))).unwrap();
    for id in ["a", "b", "c"] {
        apply(&mut ns, &add(node(id, "rect", Some("root")))).unwrap();
    }
    ns
}

#[test]
fn add_without_pos_goes_last_and_order_is_pos_not_list_position() {
    let mut ns = base_doc();
    assert_eq!(order(&ns, Some("root")), ["a", "b", "c"]);
    ns.reverse();
    assert_eq!(order(&ns, Some("root")), ["a", "b", "c"]);
}

#[test]
fn reorder_by_anchor_by_pos_and_to_the_front() {
    let mut ns = base_doc();
    apply(&mut ns, &reorder("c", Some("a"), None, None)).unwrap();
    assert_eq!(order(&ns, Some("root")), ["c", "a", "b"]);
    apply(&mut ns, &reorder("c", None, Some("b"), None)).unwrap();
    assert_eq!(order(&ns, Some("root")), ["a", "b", "c"]);
    // No anchor and no pos: to the front, which is the end of the sibling list.
    apply(&mut ns, &reorder("a", None, None, None)).unwrap();
    assert_eq!(order(&ns, Some("root")), ["b", "c", "a"]);
    let pa = pos_of(&ns, "a");
    apply(&mut ns, &reorder("a", None, None, None)).unwrap();
    assert_eq!(pos_of(&ns, "a"), pa, "already in front: nothing changes");
    // A pos that another sibling already holds lands right behind that sibling.
    let pb = pos_of(&ns, "b");
    apply(&mut ns, &reorder("c", None, None, Some(&pb))).unwrap();
    assert_eq!(order(&ns, Some("root")), ["b", "c", "a"]);
    assert_ne!(pos_of(&ns, "c"), pb);
}

#[test]
fn an_incoherent_reorder_is_refused() {
    let mut ns = base_doc();
    let before = serde_json::to_string(&ns).unwrap();
    let bad = [
        reorder("a", Some("b"), Some("c"), None),
        reorder("a", Some("root"), None, None),
        reorder("a", Some("a"), None, None),
    ];
    for cmd in bad {
        assert!(
            matches!(apply(&mut ns, &cmd), Err(ApplyError::BadReorder(_))),
            "{cmd:?}"
        );
    }
    let r = apply(&mut ns, &reorder("a", None, None, Some("!!")));
    assert!(matches!(r, Err(ApplyError::InvalidPos(_))), "{r:?}");
    assert_eq!(serde_json::to_string(&ns).unwrap(), before);
}

#[test]
fn reparent_sets_parent_and_pos_together() {
    let mut ns = base_doc();
    apply(&mut ns, &add(node("f", "frame", Some("root")))).unwrap();
    for id in ["x", "y"] {
        apply(&mut ns, &add(node(id, "rect", Some("f")))).unwrap();
    }
    let p = between(Some(&pos_of(&ns, "x")), Some(&pos_of(&ns, "y"))).unwrap();
    let into = |id: &str, pos: Option<&str>| Command::Reparent {
        id: id.into(),
        parent_id: Some("f".into()),
        pos: pos.map(Into::into),
    };
    apply(&mut ns, &into("a", Some(&p))).unwrap();
    let a = ns.iter().find(|n| n.id == "a").unwrap();
    assert_eq!(
        (a.parent_id.as_deref(), a.pos.as_str()),
        (Some("f"), p.as_str())
    );
    assert_eq!(order(&ns, Some("f")), ["x", "a", "y"]);
    // No pos: after the last child.
    apply(&mut ns, &into("b", None)).unwrap();
    assert_eq!(order(&ns, Some("f")), ["x", "a", "y", "b"]);
    // Same parent and no pos: the order stays.
    let pb = pos_of(&ns, "b");
    apply(&mut ns, &into("b", None)).unwrap();
    assert_eq!(pos_of(&ns, "b"), pb);
}

#[test]
fn concurrent_inserts_into_the_same_gap_stay_unique() {
    // Two clients computed the same pos between `a` and `b`.
    let mut ns = base_doc();
    let p = between(Some(&pos_of(&ns, "a")), Some(&pos_of(&ns, "b"))).unwrap();
    apply(&mut ns, &add(node_pos("u", Some("root"), &p))).unwrap();
    apply(&mut ns, &add(node_pos("v", Some("root"), &p))).unwrap();
    assert_eq!(order(&ns, Some("root")), ["a", "u", "v", "b", "c"]);
    let (pu, pv) = (pos_of(&ns, "u"), pos_of(&ns, "v"));
    assert_eq!(pu, p);
    assert!(pu < pv && pv < pos_of(&ns, "b"));
    // Arriving in the other order gives the other order; the server's order decides.
    let mut ns2 = base_doc();
    apply(&mut ns2, &add(node_pos("v", Some("root"), &p))).unwrap();
    apply(&mut ns2, &add(node_pos("u", Some("root"), &p))).unwrap();
    assert_eq!(order(&ns2, Some("root")), ["a", "v", "u", "b", "c"]);
}

#[test]
fn a_bad_client_pos_is_refused_everywhere_it_can_arrive() {
    let mut ns = base_doc();
    let too_long = "z".repeat(70);
    let bad = [
        "!",
        "a0 ",
        "é",
        "a10",
        "a",
        "A00000000000000000000000000",
        "a0\u{0}",
        &too_long,
    ];
    for pos in bad {
        let r = apply(&mut ns, &add(node_pos("bad", Some("root"), pos)));
        assert!(
            matches!(
                r,
                Err(ApplyError::InvalidPos(_) | ApplyError::PosTooLong { .. })
            ),
            "{pos:?}"
        );
        let to_root = Command::Reparent {
            id: "a".into(),
            parent_id: None,
            pos: Some(pos.into()),
        };
        assert!(apply(&mut ns, &to_root).is_err(), "{pos:?}");
    }
    assert_eq!(order(&ns, Some("root")), ["a", "b", "c"]);
}
