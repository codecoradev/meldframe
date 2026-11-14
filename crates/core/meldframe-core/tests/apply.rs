//! `apply_command`: validate, then change. A refused command leaves the nodes untouched, and the
//! same commands in the same order always give the same nodes.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_core::{ApplyError, Limits, apply_command};
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

fn width(id: &str, w: f64) -> Command {
    Command::UpdateNode {
        id: id.into(),
        patch: NodePatch {
            w: Some(w),
            ..NodePatch::default()
        },
    }
}

fn apply(nodes: &mut Vec<Node>, cmd: &Command) -> Result<(), ApplyError> {
    apply_command(nodes, cmd, &Limits::default())
}

fn json(nodes: &[Node]) -> String {
    serde_json::to_string(nodes).unwrap()
}

#[test]
fn add_update_and_remove_a_subtree() {
    let mut ns = vec![];
    apply(&mut ns, &add(node("a", "frame", None))).unwrap();
    apply(&mut ns, &add(node("b", "frame", Some("a")))).unwrap();
    apply(&mut ns, &add(node("c", "rect", Some("b")))).unwrap();
    apply(&mut ns, &width("c", 250.0)).unwrap();
    assert_eq!(ns[2].w, 250.0);
    apply(&mut ns, &Command::RemoveNode { id: "b".into() }).unwrap();
    assert_eq!(
        ns.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
        ["a"],
        "removing b removes c"
    );
}

#[test]
fn a_refused_command_does_not_change_the_nodes() {
    let mut ns = vec![];
    apply(&mut ns, &add(node("a", "frame", None))).unwrap();
    apply(&mut ns, &add(node("r", "rect", Some("a")))).unwrap();
    let before = json(&ns);
    let reparent = |id: &str, parent: &str| Command::Reparent {
        id: id.into(),
        parent_id: Some(parent.into()),
        pos: None,
    };
    let cases: Vec<(Command, ApplyError)> = vec![
        (
            add(node("a", "frame", None)),
            ApplyError::DuplicateId("a".into()),
        ),
        (
            add(node("x", "rect", Some("nope"))),
            ApplyError::ParentNotFound("nope".into()),
        ),
        (
            add(node("x", "rect", Some("r"))),
            ApplyError::ParentNotFrame("r".into()),
        ),
        (width("nope", 1.0), ApplyError::NotFound("nope".into())),
        (
            width("r", -1.0),
            ApplyError::OutOfRange {
                field: "w",
                value: -1.0,
            },
        ),
        (
            Command::RemoveNode { id: "nope".into() },
            ApplyError::NotFound("nope".into()),
        ),
        (reparent("a", "a"), ApplyError::Cycle),
        (
            reparent("r", "nope"),
            ApplyError::ParentNotFound("nope".into()),
        ),
        (reparent("r", "r"), ApplyError::Cycle),
    ];
    for (cmd, expected) in cases {
        assert_eq!(apply(&mut ns, &cmd), Err(expected), "{cmd:?}");
        assert_eq!(json(&ns), before, "{cmd:?} must not change the nodes");
    }
}

#[test]
fn replay_is_deterministic() {
    let cmds = vec![
        add(node("a", "frame", None)),
        add(node("r1", "rect", Some("a"))),
        width("r1", 5.0),
        add(node("r2", "rect", Some("a"))),
        Command::Reparent {
            id: "r2".into(),
            parent_id: None,
            pos: None,
        },
        Command::RemoveNode { id: "r1".into() },
    ];
    let run = || {
        let mut ns = vec![];
        for c in &cmds {
            apply(&mut ns, c).unwrap();
        }
        json(&ns)
    };
    assert_eq!(run(), run());
}
