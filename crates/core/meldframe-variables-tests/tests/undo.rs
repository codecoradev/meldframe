//! E3.3: variable commands use the same undo as node commands: one history, per-actor, exact inverses. A
//! remote variable command rewrites my steps so that my undo never overwrites what another actor did.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{add, appearance, bg, bind_fill, cmd};
use meldframe_history::FileState;
use meldframe_protocol::{Bind, Value};
use serde_json::json;

/// The collection `t`, the variable `bg` and the rect `card`.
fn file() -> FileState {
    let mut s = FileState::new();
    s.apply(&appearance()).unwrap();
    s.apply(&bg()).unwrap();
    s.apply(&add("card", "rect", None)).unwrap();
    s
}

fn value(s: &FileState, id: &str, mode: &str) -> Bind<Value> {
    s.vars().variable(id).unwrap().values[mode].clone()
}

fn color(c: &str) -> Bind<Value> {
    Bind::Lit(Value::Color(c.into()))
}

#[test]
fn undo_and_redo_walk_back_through_variable_and_node_commands_in_one_history() {
    let mut s = file();
    let steps = s.undo_depth();
    s.apply(&cmd(
        json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": "#111111"}),
    ))
    .unwrap();
    s.apply(&bind_fill("card", "bg")).unwrap();
    assert_eq!(s.undo_depth(), steps + 2);
    // The binding is undone first, then the value; the variable and the node are as they were.
    assert!(s.undo().is_some());
    assert_eq!(s.node("card").unwrap().fill, "#3f3f46");
    assert!(s.undo().is_some());
    assert_eq!(value(&s, "bg", "dark"), color("#000000"));
    assert!(s.redo().is_some() && s.redo().is_some());
    assert_eq!(value(&s, "bg", "dark"), color("#111111"));
    assert_eq!(s.node("card").unwrap().fill, Bind::var("bg"));
}

#[test]
fn undoing_the_add_of_a_variable_that_another_actor_bound_is_skipped() {
    let mut s = FileState::new();
    for c in [
        json!({"type": "addCollection", "collection": {"id": "t", "name": "appearance", "modes": [{"id": "m", "name": "m"}]}}),
        json!({"type": "addVariable", "variable": {"id": "bg", "collection": "t", "name": "color/bg", "type": "color",
            "values": {"m": "#ffffff"}}}),
    ] {
        s.apply(&cmd(c)).unwrap();
    }
    s.apply_remote(&add("card", "rect", None)).unwrap();
    s.apply_remote(&bind_fill("card", "bg")).unwrap();
    // Removing `bg` is refused while the card uses it, and so is removing its collection: nothing is undone and
    // nothing is lost from the file.
    assert!(s.undo().is_none());
    assert!(s.vars().variable("bg").is_some());
    assert_eq!(s.node("card").unwrap().fill, Bind::var("bg"));
}

#[test]
fn undoing_the_add_of_an_unused_variable_removes_it_and_redo_brings_it_back() {
    let mut s = file();
    assert!(s.undo().is_some(), "the card goes first");
    assert!(s.undo().is_some());
    assert!(s.vars().variable("bg").is_none());
    assert!(s.redo().is_some());
    assert!(s.vars().variable("bg").is_some());
}

#[test]
fn undo_of_a_removed_mode_brings_back_the_mode_with_its_values_in_place() {
    let mut s = file();
    s.apply(&cmd(
        json!({"type": "addMode", "collection": "t", "mode": {"id": "hc", "name": "contrast"}}),
    ))
    .unwrap();
    s.apply(&cmd(
        json!({"type": "setVariableValue", "id": "bg", "mode": "hc", "value": "#ff00ff"}),
    ))
    .unwrap();
    s.apply(&cmd(
        json!({"type": "removeMode", "collection": "t", "mode": "dark"}),
    ))
    .unwrap();
    assert!(s.undo().is_some());
    let modes: Vec<&str> = s
        .vars()
        .collection("t")
        .unwrap()
        .modes
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(modes, ["light", "dark", "hc"]);
    assert_eq!(value(&s, "bg", "dark"), color("#000000"));
    assert_eq!(value(&s, "bg", "hc"), color("#ff00ff"));
}

#[test]
fn undo_keeps_the_value_another_actor_set_after_me() {
    let mut s = file();
    s.apply(&cmd(
        json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": "#111111"}),
    ))
    .unwrap();
    s.apply(&cmd(
        json!({"type": "setVariableValue", "id": "bg", "mode": "light", "value": "#eeeeee"}),
    ))
    .unwrap();
    // The other actor writes the dark value after me.
    s.apply_remote(&cmd(
        json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": "#222222"}),
    ))
    .unwrap();
    assert!(s.undo().is_some());
    assert_eq!(
        value(&s, "bg", "light"),
        color("#ffffff"),
        "my light value is restored"
    );
    assert!(
        s.undo().is_some(),
        "the step that only held the dark value is gone, the one below it is undone"
    );
    assert!(
        s.node("card").is_none(),
        "the step below it is the add of the card, and that is the one that went"
    );
    assert!(s.vars().variable("bg").is_some());
    assert_eq!(
        value(&s, "bg", "dark"),
        color("#222222"),
        "the other actor's dark value stays"
    );
}

#[test]
fn a_step_on_a_variable_that_another_actor_removed_goes() {
    let mut s = file();
    s.apply(&cmd(
        json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": "#111111"}),
    ))
    .unwrap();
    let depth = s.undo_depth();
    s.apply_remote(&cmd(json!({"type": "removeVariable", "id": "bg"})))
        .unwrap();
    assert_eq!(s.undo_depth(), depth - 1);
    assert!(s.vars().variable("bg").is_none());
}

/// The collection `t` and `bg`, with a rect `card` and the binding of its fill to `bg` made by another actor.
fn bound_by_the_other_actor() -> FileState {
    let mut s = FileState::new();
    s.apply(&appearance()).unwrap();
    s.apply(&bg()).unwrap();
    s.apply_remote(&add("card", "rect", None)).unwrap();
    s.apply_remote(&bind_fill("card", "bg")).unwrap();
    s
}

fn literal_fill(id: &str, c: &str) -> meldframe_protocol::Command {
    cmd(json!({"type": "updateNode", "id": id, "patch": {"fill": c}}))
}

#[test]
fn an_undo_that_would_bind_a_variable_the_other_actor_removed_does_not_block_the_stack() {
    let mut s = bound_by_the_other_actor();
    // I paint the card red: my undo puts the binding back. Nothing uses `bg` then, and the other actor removes it.
    s.apply(&literal_fill("card", "#ff0000")).unwrap();
    s.apply_remote(&cmd(json!({"type": "removeVariable", "id": "bg"})))
        .unwrap();
    // The undo of the red fill would name a variable that is gone: it changes nothing instead of being refused
    // for ever, and the steps below it are not stuck behind it.
    // One undo passes the steps that have nothing left to undo and takes the add of the collection.
    assert!(s.undo().is_some(), "the red fill's undo is not refused");
    assert_eq!(s.node("card").unwrap().fill, "#ff0000");
    assert!(
        s.vars().collection("t").is_none(),
        "the stack is not blocked"
    );
    assert_eq!(s.undo_depth(), 0);
}

#[test]
fn undoing_the_delete_of_a_bound_node_brings_it_back_unbound_when_the_variable_is_gone() {
    let mut s = bound_by_the_other_actor();
    s.apply(&cmd(json!({"type": "removeNode", "id": "card"})))
        .unwrap();
    s.apply_remote(&cmd(json!({"type": "removeVariable", "id": "bg"})))
        .unwrap();
    assert!(s.undo().is_some(), "the delete is undone");
    assert_eq!(
        s.node("card").unwrap().fill,
        "#3f3f46",
        "the card comes back with the default fill, not with a binding to nothing"
    );
}

#[test]
fn an_undo_that_would_alias_a_variable_the_other_actor_removed_does_not_block_the_stack() {
    let mut s = FileState::new();
    s.apply(&appearance()).unwrap();
    s.apply(&bg()).unwrap();
    s.apply_remote(&cmd(
        json!({"type": "addVariable", "variable": {"id": "y", "collection": "t", "name": "color/y", "type": "color",
            "values": {"light": "#123456", "dark": "#654321"}}}),
    ))
    .unwrap();
    s.apply_remote(&cmd(
        json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": {"var": "y"}}),
    ))
    .unwrap();
    s.apply(&cmd(
        json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": "#111111"}),
    ))
    .unwrap();
    s.apply_remote(&cmd(json!({"type": "removeVariable", "id": "y"})))
        .unwrap();
    // The undo that named `y` has nothing left to do and is passed; the add of `bg` below it is undone.
    assert!(s.undo().is_some(), "the undo that named `y` is not refused");
    assert!(
        s.vars().variable("bg").is_none(),
        "the stack is not blocked"
    );
    assert!(s.vars().collection("t").is_some());
}

#[test]
fn a_mode_another_actor_adds_copies_the_default_mode_into_the_variable_i_bring_back() {
    let mut s = FileState::new();
    // The default mode `zulu` sorts after `alpha`: the copy must come from it, not from the first key.
    for c in [
        json!({"type": "addCollection", "collection": {"id": "t", "name": "appearance",
            "modes": [{"id": "zulu", "name": "zulu"}, {"id": "alpha", "name": "alpha"}]}}),
        json!({"type": "addVariable", "variable": {"id": "bg", "collection": "t", "name": "color/bg", "type": "color",
            "values": {"zulu": "#111111", "alpha": "#222222"}}}),
        json!({"type": "removeVariable", "id": "bg"}),
    ] {
        s.apply(&cmd(c)).unwrap();
    }
    s.apply_remote(&cmd(
        json!({"type": "addMode", "collection": "t", "mode": {"id": "mid", "name": "mid"}}),
    ))
    .unwrap();
    assert!(s.undo().is_some());
    assert_eq!(
        value(&s, "bg", "mid"),
        color("#111111"),
        "copied from the default mode"
    );
    assert_eq!(value(&s, "bg", "alpha"), color("#222222"));
}
