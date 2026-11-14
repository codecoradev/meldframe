//! Loading a stored file checks it: a bad file is refused with a typed error and a stable code, never
//! repaired. A good file loads unchanged.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{add, node, snap};
use meldframe_check::Limits;
use meldframe_history::FileState;
use meldframe_protocol::Node;

fn load(nodes: Vec<Node>) -> Result<FileState, String> {
    FileState::try_from_nodes(nodes, Limits::default()).map_err(|e| e.code().to_string())
}

fn refused(nodes: Vec<Node>) -> String {
    load(nodes).expect_err("the file must be refused")
}

fn good() -> Vec<Node> {
    let mut s = FileState::new();
    s.apply(&add("root", "frame", None)).unwrap();
    s.apply(&add("p", "frame", Some("root"))).unwrap();
    for id in ["a", "b", "c"] {
        s.apply(&add(id, "rect", Some("p"))).unwrap();
    }
    s.nodes().to_vec()
}

#[test]
fn a_good_file_loads_unchanged_and_its_output_loads_to_an_identical_snapshot() {
    let first = load(good()).unwrap();
    assert_eq!(first.version(), 0);
    let second = load(first.nodes().to_vec()).unwrap();
    assert_eq!(snap(&second), snap(&first));
    let json = |v: &[Node]| serde_json::to_string(v).unwrap();
    assert_eq!(json(first.nodes()), json(&good()));
}

#[test]
fn an_old_file_without_pos_is_migrated_not_refused() {
    let old = vec![
        node("root", "frame", None),
        node("b", "rect", Some("root")),
        node("a", "rect", Some("root")),
    ];
    let s = load(old).unwrap();
    assert!(s.nodes().iter().all(|n| !n.pos.is_empty()));
}

#[test]
fn a_duplicate_id_is_refused() {
    let mut v = good();
    v.push(node("a", "rect", Some("p")));
    assert_eq!(refused(v), "E_DUPLICATE_ID");
}

#[test]
fn a_missing_parent_is_refused() {
    let mut v = good();
    v.push(node("x", "rect", Some("ghost")));
    assert_eq!(refused(v), "E_PARENT_NOT_FOUND");
}

#[test]
fn a_parent_that_is_not_a_frame_is_refused() {
    let mut v = good();
    v.push(node("x", "rect", Some("a")));
    assert_eq!(refused(v), "E_PARENT_NOT_FRAME");
}

#[test]
fn a_text_node_with_children_is_refused() {
    let v = vec![
        node("root", "frame", None),
        node("t", "text", Some("root")),
        node("k", "rect", Some("t")),
    ];
    assert_eq!(refused(v), "E_PARENT_NOT_FRAME");
}

#[test]
fn a_cycle_is_refused() {
    let v = vec![
        node("root", "frame", None),
        node("a", "frame", Some("b")),
        node("b", "frame", Some("a")),
    ];
    assert_eq!(refused(v), "E_CYCLE");
}

#[test]
fn a_node_that_is_its_own_parent_is_refused() {
    let v = vec![node("root", "frame", None), node("a", "frame", Some("a"))];
    assert_eq!(refused(v), "E_CYCLE");
}

#[test]
fn a_tree_deeper_than_the_limit_is_refused() {
    let limits = Limits {
        max_depth: 2,
        ..Limits::default()
    };
    let v = vec![
        node("n0", "frame", None),
        node("n1", "frame", Some("n0")),
        node("n2", "frame", Some("n1")),
        node("n3", "rect", Some("n2")),
    ];
    let err = FileState::try_from_nodes(v.clone(), limits.clone()).unwrap_err();
    assert_eq!(err.code(), "E_TOO_DEEP");
    assert!(FileState::try_from_nodes(v[..3].to_vec(), limits).is_ok());
}

#[test]
fn more_nodes_than_the_limit_are_refused() {
    let limits = Limits {
        max_nodes: 2,
        ..Limits::default()
    };
    let v = vec![
        node("root", "frame", None),
        node("a", "rect", Some("root")),
        node("b", "rect", Some("root")),
    ];
    let err = FileState::try_from_nodes(v, limits).unwrap_err();
    assert_eq!(err.code(), "E_TOO_MANY_NODES");
}

#[test]
fn a_bad_colour_is_refused_and_the_error_names_the_node() {
    let mut v = good();
    let i = v.iter().position(|n| n.id == "b").unwrap();
    v[i].fill = "red".into();
    let err = FileState::try_from_nodes(v, Limits::default()).unwrap_err();
    assert_eq!(err.code(), "E_INVALID_COLOR");
    assert!(err.to_string().contains('b'), "{err}");
}

#[test]
fn an_invalid_or_repeated_pos_is_migrated_by_the_old_file_rule_not_refused() {
    let mut v = good();
    let b = v.iter().position(|n| n.id == "b").unwrap();
    let c = v.iter().position(|n| n.id == "c").unwrap();
    v[b].pos = "not a pos!".into();
    v[c].pos = v[b].pos.clone();
    let s = load(v).unwrap();
    assert!(meldframe_check::check_file(s.nodes(), &Limits::default()).is_ok());
}

#[test]
fn the_migration_uses_the_limits_of_the_loader() {
    // A long but valid pos: the loader with a larger limit keeps it, with a smaller limit it re-keys the
    // group instead of passing a pos that the check would refuse next.
    let long = |id: &str, pos: &str| {
        let mut n = node(id, "rect", Some("root"));
        n.pos = pos.to_string();
        n
    };
    let (a, b) = ("V".repeat(70) + "1", "V".repeat(70) + "2");
    let file = || vec![node("root", "frame", None), long("a", &a), long("b", &b)];
    let wide = Limits {
        max_pos: 100,
        ..Limits::default()
    };
    let kept = FileState::try_from_nodes(file(), wide).expect("a valid long pos loads");
    assert!(kept.nodes().iter().any(|n| n.pos == a) && kept.nodes().iter().any(|n| n.pos == b));
    let narrow = Limits {
        max_pos: 8,
        ..Limits::default()
    };
    let fixed = FileState::try_from_nodes(file(), narrow).expect("a too long pos is migrated");
    assert!(fixed.nodes().iter().all(|n| n.pos.len() <= 8));
}
