//! `FileState::apply`: a command that passes validation changes the tree and bumps the version; a
//! refused command changes nothing.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_history::{ApplyError, FileState, Limits};
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

fn ids(s: &FileState) -> Vec<&str> {
    s.nodes().iter().map(|n| n.id.as_str()).collect()
}

fn json(s: &FileState) -> String {
    serde_json::to_string(s.nodes()).unwrap()
}

#[test]
fn a_new_file_state_is_empty_at_version_zero() {
    let s = FileState::new();
    assert!(s.nodes().is_empty());
    assert_eq!(s.version(), 0);
    assert!(s.node("a").is_none());
}

#[test]
fn apply_changes_the_tree_and_bumps_the_version_each_time() {
    let mut s = FileState::new();
    s.apply(&add(node("a", "frame", None))).unwrap();
    s.apply(&add(node("b", "rect", Some("a")))).unwrap();
    assert_eq!(s.version(), 2);
    s.apply(&width("b", 250.0)).unwrap();
    assert_eq!(s.node("b").unwrap().w, 250.0);
    assert_eq!(s.version(), 3);
    s.apply(&Command::RemoveNode { id: "a".into() }).unwrap();
    assert!(ids(&s).is_empty(), "removing a removes b");
    assert_eq!(s.version(), 4);
}

#[test]
fn a_refused_command_changes_neither_the_tree_nor_the_version() {
    let mut s = FileState::new();
    s.apply(&add(node("a", "rect", None))).unwrap();
    let (before, version) = (json(&s), s.version());
    let err = s.apply(&add(node("a", "rect", None))).unwrap_err();
    assert_eq!(err, ApplyError::DuplicateId("a".into()));
    assert_eq!(
        s.apply(&Command::RemoveNode { id: "nope".into() }),
        Err(ApplyError::NotFound("nope".into()))
    );
    assert_eq!(json(&s), before);
    assert_eq!(s.version(), version);
}

#[test]
fn from_nodes_unchecked_starts_at_version_zero_with_the_given_tree() {
    let mut seed = FileState::new();
    seed.apply(&add(node("root", "frame", None))).unwrap();
    seed.apply(&add(node("a", "rect", Some("root")))).unwrap();
    let loaded = FileState::from_nodes_unchecked(seed.nodes().to_vec());
    assert_eq!(json(&loaded), json(&seed));
    assert_eq!(loaded.version(), 0);
}

#[test]
fn tighter_limits_refuse_what_the_defaults_allow() {
    let limits = Limits {
        max_nodes: 1,
        ..Limits::default()
    };
    let mut s = FileState::with_limits(limits);
    s.apply(&add(node("a", "rect", None))).unwrap();
    assert_eq!(
        s.apply(&add(node("b", "rect", None))),
        Err(ApplyError::TooManyNodes(1))
    );
    assert_eq!(ids(&s), ["a"]);
}

#[test]
fn the_same_commands_in_the_same_order_give_the_same_tree() {
    let cmds = [
        add(node("a", "frame", None)),
        add(node("b", "rect", Some("a"))),
        add(node("c", "rect", Some("a"))),
        width("c", 40.0),
        Command::Reorder {
            id: "c".into(),
            before: Some("b".into()),
            after: None,
            pos: None,
        },
    ];
    let (mut one, mut two) = (FileState::new(), FileState::new());
    for c in &cmds {
        one.apply(c).unwrap();
        two.apply(c).unwrap();
    }
    assert_eq!(json(&one), json(&two));
}
