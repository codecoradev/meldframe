//! Semantic validation of a command against the nodes it would change. `check_command` never changes
//! the nodes; a refusal names its reason.

use meldframe_check::{ApplyError, Limits, check_command};
use meldframe_protocol::{Command, Node, NodePatch};

fn node(id: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent})).unwrap()
}

fn add(n: Node) -> Command {
    Command::AddNode {
        node: n,
        index: None,
    }
}

fn reparent(id: &str, parent: Option<&str>, pos: Option<&str>) -> Command {
    Command::Reparent {
        id: id.into(),
        parent_id: parent.map(Into::into),
        pos: pos.map(Into::into),
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

/// Frame `a` holding frame `b` (holding rect `c`), plus rect `r` directly under `a`.
fn doc() -> Vec<Node> {
    vec![
        node("a", "frame", None),
        node("b", "frame", Some("a")),
        node("c", "rect", Some("b")),
        node("r", "rect", Some("a")),
    ]
}

fn check(nodes: &[Node], cmd: &Command) -> Result<(), ApplyError> {
    check_command(nodes, cmd, &Limits::default())
}

#[test]
fn accepts_well_formed_commands() {
    let ns = doc();
    assert_eq!(check(&ns, &add(node("x", "rect", Some("a")))), Ok(()));
    assert_eq!(check(&ns, &add(node("x", "frame", None))), Ok(()));
    let update = Command::UpdateNode {
        id: "c".into(),
        patch: NodePatch::default(),
    };
    assert_eq!(check(&ns, &update), Ok(()));
    assert_eq!(check(&ns, &Command::RemoveNode { id: "b".into() }), Ok(()));
    assert_eq!(check(&ns, &reparent("c", None, None)), Ok(()));
    assert_eq!(check(&ns, &reorder("r", Some("b"), None, None)), Ok(()));
    assert_eq!(check(&ns, &reorder("r", None, None, Some("a0"))), Ok(()));
}

#[test]
fn refuses_unknown_nodes() {
    let ns = doc();
    let update = Command::UpdateNode {
        id: "nope".into(),
        patch: NodePatch::default(),
    };
    assert_eq!(
        check(&ns, &update),
        Err(ApplyError::NotFound("nope".into()))
    );
    let remove = Command::RemoveNode { id: "nope".into() };
    assert_eq!(
        check(&ns, &remove),
        Err(ApplyError::NotFound("nope".into()))
    );
    assert_eq!(
        check(&ns, &reparent("nope", None, None)),
        Err(ApplyError::NotFound("nope".into()))
    );
    let r = reorder("nope", None, None, None);
    assert_eq!(check(&ns, &r), Err(ApplyError::NotFound("nope".into())));
}

#[test]
fn refuses_a_duplicate_id_and_a_bad_parent() {
    let ns = doc();
    assert_eq!(
        check(&ns, &add(node("a", "frame", None))),
        Err(ApplyError::DuplicateId("a".into()))
    );
    let orphan = add(node("x", "rect", Some("nope")));
    assert_eq!(
        check(&ns, &orphan),
        Err(ApplyError::ParentNotFound("nope".into()))
    );
    let under_rect = add(node("x", "rect", Some("r")));
    assert_eq!(
        check(&ns, &under_rect),
        Err(ApplyError::ParentNotFrame("r".into()))
    );
    assert_eq!(
        check(&ns, &reparent("c", Some("nope"), None)),
        Err(ApplyError::ParentNotFound("nope".into()))
    );
    assert_eq!(
        check(&ns, &reparent("c", Some("r"), None)),
        Err(ApplyError::ParentNotFrame("r".into()))
    );
}

#[test]
fn validates_the_node_and_the_patch_it_carries() {
    let ns = doc();
    let mut bad = node("x", "rect", None);
    bad.fill = "red".into();
    assert_eq!(
        check(&ns, &add(bad)),
        Err(ApplyError::InvalidColor { field: "fill" })
    );
    let patch = NodePatch {
        w: Some(-1.0),
        ..NodePatch::default()
    };
    let update = Command::UpdateNode {
        id: "r".into(),
        patch,
    };
    assert!(matches!(
        check(&ns, &update),
        Err(ApplyError::OutOfRange { field: "w", .. })
    ));
}

#[test]
fn refuses_a_reparent_that_would_make_a_cycle() {
    let ns = doc();
    assert_eq!(
        check(&ns, &reparent("a", Some("a"), None)),
        Err(ApplyError::Cycle)
    );
    // `b` is a child of `a`, so `a` cannot move under it.
    assert_eq!(
        check(&ns, &reparent("a", Some("b"), None)),
        Err(ApplyError::Cycle)
    );
    assert_eq!(
        check(&ns, &reparent("b", Some("b"), None)),
        Err(ApplyError::Cycle)
    );
    assert_eq!(check(&ns, &reparent("b", None, None)), Ok(()));
}

#[test]
fn enforces_the_node_count_limit() {
    let l = Limits {
        max_nodes: 4,
        ..Limits::default()
    };
    let ns = doc();
    let e = check_command(&ns, &add(node("x", "rect", Some("a"))), &l);
    assert_eq!(e, Err(ApplyError::TooManyNodes(4)));
    let l = Limits {
        max_nodes: 5,
        ..Limits::default()
    };
    assert_eq!(
        check_command(&ns, &add(node("x", "rect", Some("a"))), &l),
        Ok(())
    );
}

#[test]
fn enforces_the_depth_limit_on_add_and_on_a_moved_subtree() {
    let l = Limits {
        max_depth: 2,
        ..Limits::default()
    };
    // `c` is at depth 2 (a > b > c): fine. A child of `b` would be at depth 2, a child of a node at depth 2 at 3.
    let mut ns = vec![
        node("a", "frame", None),
        node("b", "frame", Some("a")),
        node("c", "frame", Some("b")),
    ];
    assert_eq!(
        check_command(&ns, &add(node("x", "rect", Some("b"))), &l),
        Ok(())
    );
    let too_deep = check_command(&ns, &add(node("d", "frame", Some("c"))), &l);
    assert_eq!(too_deep, Err(ApplyError::TooDeep(2)));
    // Moving `a` (height 2) under another root frame would put its deepest node at depth 3.
    ns.push(node("z", "frame", None));
    let moved = check_command(&ns, &reparent("a", Some("z"), None), &l);
    assert_eq!(moved, Err(ApplyError::TooDeep(2)));
    // Moving only `b` (height 1) under `z` keeps the deepest node at depth 2.
    assert_eq!(
        check_command(&ns, &reparent("b", Some("z"), None), &l),
        Ok(())
    );
}

#[test]
fn reorder_takes_at_most_one_of_before_after_and_pos() {
    let ns = doc();
    let two = reorder("r", Some("b"), Some("b"), None);
    assert!(matches!(check(&ns, &two), Err(ApplyError::BadReorder(_))));
    let three = reorder("r", Some("b"), None, Some("a0"));
    assert!(matches!(check(&ns, &three), Err(ApplyError::BadReorder(_))));
}

#[test]
fn reorder_anchors_must_be_other_siblings() {
    let ns = doc();
    // missing anchor, itself, a node under another parent, and the parent
    for anchor in ["nope", "r", "c", "a"] {
        let e = check(&ns, &reorder("r", Some(anchor), None, None));
        assert!(
            matches!(e, Err(ApplyError::BadReorder(_))),
            "before {anchor}: {e:?}"
        );
        let e = check(&ns, &reorder("r", None, Some(anchor), None));
        assert!(
            matches!(e, Err(ApplyError::BadReorder(_))),
            "after {anchor}: {e:?}"
        );
    }
}

#[test]
fn a_client_pos_is_validated_on_add_reorder_and_reparent() {
    let ns = doc();
    for bad in ["!", "a10", "A00000000000000000000000000", &"z".repeat(70)] {
        let mut n = node("x", "rect", Some("a"));
        n.pos = bad.into();
        assert!(check(&ns, &add(n)).is_err(), "add {bad:?}");
        assert!(
            check(&ns, &reorder("r", None, None, Some(bad))).is_err(),
            "reorder {bad:?}"
        );
        assert!(
            check(&ns, &reparent("c", None, Some(bad))).is_err(),
            "reparent {bad:?}"
        );
    }
    let e = check(&ns, &reorder("r", None, None, Some("a10")));
    assert!(matches!(e, Err(ApplyError::InvalidPos(_))), "{e:?}");
}

#[test]
fn commands_with_unknown_fields_do_not_parse() {
    assert!(serde_json::from_str::<Command>(r#"{"type":"reorder","id":"a","pos":5}"#).is_err());
    assert!(serde_json::from_str::<Command>(r#"{"type":"reorder","id":"a","zzz":1}"#).is_err());
    assert!(serde_json::from_str::<Command>(r#"{"type":"reorder","id":"a","before":"b"}"#).is_ok());
}
