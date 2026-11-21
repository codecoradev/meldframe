//! `check_edit_at`: the check of a list of variable commands that says where the list fails, so a caller can name what
//! the failing command is about. It is `check_edit` with the place of the refusal.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app_rules::modes::{EditRefusal, check_edit, check_edit_at};
use meldframe_protocol::{Command, Node, Variables};
use serde_json::json;

fn cmds(v: serde_json::Value) -> Vec<Command> {
    serde_json::from_value(v).unwrap()
}

fn file() -> Variables {
    serde_json::from_value(json!({
        "collections": [{"id": "c0", "name": "mine", "modes": [{"id": "m0", "name": "default"}]}],
        "variables": [{"id": "taken", "collection": "c0", "name": "x", "type": "number", "values": {"m0": 1}}]
    }))
    .unwrap()
}

fn list() -> Vec<Command> {
    cmds(json!([
        {"type": "addCollection", "collection": {"id": "c1", "name": "new", "modes": [{"id": "m1", "name": "default"}]}},
        {"type": "addVariable", "variable": {"id": "ok", "collection": "c1", "name": "a", "type": "number", "values": {"m1": 1}}},
        {"type": "addVariable", "variable": {"id": "taken", "collection": "c1", "name": "b", "type": "number", "values": {"m1": 2}}},
        {"type": "addVariable", "variable": {"id": "late", "collection": "c1", "name": "c", "type": "number", "values": {"m1": 3}}}
    ]))
}

#[test]
fn a_list_that_fits_is_ok() {
    let nodes: Vec<Node> = Vec::new();
    assert!(check_edit_at(&nodes, &file(), &list()[..2]).is_ok());
}

#[test]
fn the_refusal_is_the_place_of_the_first_command_that_fails_and_it_is_above_zero() {
    let nodes: Vec<Node> = Vec::new();
    let (at, e) = check_edit_at(&nodes, &file(), &list()).unwrap_err();
    assert_eq!(
        at, 2,
        "the first two are fine, the third has an id that is taken"
    );
    match e {
        EditRefusal::Invalid(e) => assert_eq!(e.code(), "E_DUPLICATE_ID"),
        EditRefusal::NotVariable => panic!("a variable command"),
    }
}

#[test]
fn a_command_checks_on_what_the_ones_before_it_made() {
    // The second command needs the collection the first makes: alone it is refused at 0, after it it is fine.
    let nodes: Vec<Node> = Vec::new();
    let two = list();
    assert_eq!(check_edit_at(&nodes, &file(), &two[1..2]).unwrap_err().0, 0);
    assert!(check_edit_at(&nodes, &file(), &two[..2]).is_ok());
}

#[test]
fn a_command_about_nodes_is_not_a_variable_command_and_has_its_place() {
    let nodes: Vec<Node> = Vec::new();
    let mut l = list();
    l.insert(1, Command::RemoveNode { id: "n".into() });
    let (at, e) = check_edit_at(&nodes, &file(), &l).unwrap_err();
    assert_eq!(at, 1);
    assert!(matches!(e, EditRefusal::NotVariable));
}

#[test]
fn check_edit_is_check_edit_at_without_the_place() {
    let nodes: Vec<Node> = Vec::new();
    let a = check_edit(&nodes, &file(), &list()).unwrap_err();
    let (_, b) = check_edit_at(&nodes, &file(), &list()).unwrap_err();
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
}
