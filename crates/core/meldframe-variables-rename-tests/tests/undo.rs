//! Per-actor undo of a rename (D8). The unit of last writer wins is the name of the entity: a remote rename of the
//! same entity, or its removal or re-creation, drops my step; a rename of another entity leaves it.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-variables-tests/tests/common/mod.rs"]
mod common;

use common::{appearance, bg, cmd};
use meldframe_history::FileState;
use meldframe_protocol::Command;
use serde_json::json;

fn rename_collection(id: &str, name: &str) -> Command {
    cmd(json!({"type": "renameCollection", "id": id, "name": name}))
}

fn rename_mode(collection: &str, mode: &str, name: &str) -> Command {
    cmd(json!({"type": "renameMode", "collection": collection, "mode": mode, "name": name}))
}

fn rename_variable(id: &str, name: &str) -> Command {
    cmd(json!({"type": "renameVariable", "id": id, "name": name}))
}

/// Collection `t` (light, dark) with colours `bg` and `fg`, and a second collection `u`, all made by another
/// actor: my history starts empty, so a test counts only the steps it makes.
fn file() -> FileState {
    let mut s = FileState::new();
    for c in setup() {
        s.apply_remote(&c).unwrap();
    }
    s
}

fn setup() -> Vec<Command> {
    vec![
        appearance(),
        bg(),
        cmd(
            json!({"type": "addVariable", "variable": {"id": "fg", "collection": "t", "name": "color/fg",
            "type": "color", "values": {"light": "#000000", "dark": "#ffffff"}}}),
        ),
        cmd(
            json!({"type": "addCollection", "collection": {"id": "u", "name": "other", "modes": [{"id": "m", "name": "m"}]}}),
        ),
    ]
}

fn var_name(s: &FileState, id: &str) -> String {
    s.vars().variable(id).unwrap().name.clone()
}

fn col_name(s: &FileState, id: &str) -> String {
    s.vars().collection(id).unwrap().name.clone()
}

fn mode_name(s: &FileState, c: &str, m: &str) -> String {
    let col = s.vars().collection(c).unwrap();
    col.modes.iter().find(|x| x.id == m).unwrap().name.clone()
}

#[test]
fn undo_and_redo_walk_a_rename_back_and_forth() {
    let mut s = file();
    s.apply(&rename_variable("bg", "color/surface")).unwrap();
    s.apply(&rename_collection("t", "theme")).unwrap();
    s.apply(&rename_mode("t", "dark", "Night")).unwrap();
    assert!(s.undo().is_some());
    assert_eq!(mode_name(&s, "t", "dark"), "dark");
    assert!(s.undo().is_some());
    assert_eq!(col_name(&s, "t"), "appearance");
    assert!(s.undo().is_some());
    assert_eq!(var_name(&s, "bg"), "color/bg");
    assert!(s.redo().is_some() && s.redo().is_some() && s.redo().is_some());
    assert_eq!(
        (
            var_name(&s, "bg"),
            col_name(&s, "t"),
            mode_name(&s, "t", "dark")
        ),
        ("color/surface".into(), "theme".into(), "Night".into())
    );
}

#[test]
fn undoing_the_add_of_a_renamed_variable_still_removes_it() {
    let mut s = FileState::new();
    for c in setup() {
        s.apply(&c).unwrap();
    }
    s.apply(&rename_variable("fg", "color/text")).unwrap();
    assert!(s.undo().is_some());
    assert_eq!(var_name(&s, "fg"), "color/fg");
    assert!(
        s.undo().is_some() && s.undo().is_some(),
        "the steps that added `fg` and `u`"
    );
    assert!(s.vars().variable("fg").is_none());
}

#[test]
fn a_remote_rename_of_the_same_entity_drops_my_undo_and_my_redo() {
    let mut s = file();
    s.apply(&rename_variable("bg", "color/surface")).unwrap();
    s.apply(&rename_collection("t", "theme")).unwrap();
    s.apply(&rename_mode("t", "dark", "Night")).unwrap();
    let depth = s.undo_depth();
    s.apply_remote(&rename_variable("bg", "color/panel"))
        .unwrap();
    s.apply_remote(&rename_collection("t", "ui")).unwrap();
    s.apply_remote(&rename_mode("t", "dark", "Dim")).unwrap();
    // The three steps hold nothing that would not overwrite the other actor: they are pruned.
    assert_eq!((depth, s.undo_depth()), (3, 0));
    assert!(s.undo().is_none(), "nothing is renamed back");
    assert_eq!(
        (
            var_name(&s, "bg"),
            col_name(&s, "t"),
            mode_name(&s, "t", "dark")
        ),
        ("color/panel".into(), "ui".into(), "Dim".into())
    );
}

#[test]
fn a_remote_rename_after_my_undo_drops_my_redo() {
    let mut s = file();
    s.apply(&rename_variable("bg", "color/surface")).unwrap();
    assert!(s.undo().is_some());
    assert_eq!(s.redo_depth(), 1);
    s.apply_remote(&rename_variable("bg", "color/panel"))
        .unwrap();
    assert_eq!(s.redo_depth(), 0);
    assert!(s.redo().is_none());
    assert_eq!(var_name(&s, "bg"), "color/panel");
}

#[test]
fn a_remote_rename_of_another_entity_leaves_my_step() {
    let mut s = file();
    s.apply(&rename_variable("bg", "color/surface")).unwrap();
    s.apply(&rename_collection("t", "theme")).unwrap();
    s.apply(&rename_mode("t", "dark", "Night")).unwrap();
    s.apply_remote(&rename_variable("fg", "color/text"))
        .unwrap();
    s.apply_remote(&rename_collection("u", "scratch")).unwrap();
    s.apply_remote(&rename_mode("t", "light", "Day")).unwrap();
    assert!(s.undo().is_some() && s.undo().is_some() && s.undo().is_some());
    assert_eq!(
        (
            var_name(&s, "bg"),
            col_name(&s, "t"),
            mode_name(&s, "t", "dark")
        ),
        ("color/bg".into(), "appearance".into(), "dark".into())
    );
    // What the other actor wrote stays.
    assert_eq!(
        (
            var_name(&s, "fg"),
            col_name(&s, "u"),
            mode_name(&s, "t", "light")
        ),
        ("color/text".into(), "scratch".into(), "Day".into())
    );
}

#[test]
fn a_remote_removal_of_the_renamed_entity_drops_my_step_and_nothing_fails() {
    let mut s = file();
    s.apply(&rename_variable("fg", "color/text")).unwrap();
    s.apply(&rename_mode("t", "dark", "Night")).unwrap();
    s.apply(&rename_collection("u", "scratch")).unwrap();
    assert_eq!(
        (
            var_name(&s, "fg"),
            mode_name(&s, "t", "dark"),
            col_name(&s, "u")
        ),
        ("color/text".into(), "Night".into(), "scratch".into())
    );
    let depth = s.undo_depth();
    s.apply_remote(&cmd(json!({"type": "removeVariable", "id": "fg"})))
        .unwrap();
    s.apply_remote(&cmd(
        json!({"type": "removeMode", "collection": "t", "mode": "dark"}),
    ))
    .unwrap();
    s.apply_remote(&cmd(json!({"type": "removeCollection", "id": "u"})))
        .unwrap();
    assert_eq!(
        (depth, s.undo_depth()),
        (3, 0),
        "the three steps would do nothing"
    );
    s.apply_remote(&rename_collection("t", "theme")).unwrap();
    // Nothing of mine is left to undo, and nothing failed.
    assert!(s.undo().is_none());
}

#[test]
fn a_remote_removal_and_re_creation_of_the_entity_drops_my_step() {
    let mut s = file();
    s.apply(&rename_variable("fg", "color/text")).unwrap();
    s.apply(&rename_mode("t", "dark", "Night")).unwrap();
    assert_eq!(
        (var_name(&s, "fg"), mode_name(&s, "t", "dark")),
        ("color/text".into(), "Night".into())
    );
    s.apply_remote(&cmd(json!({"type": "removeVariable", "id": "fg"})))
        .unwrap();
    s.apply_remote(&cmd(json!({"type": "addVariable", "variable": {"id": "fg", "collection": "t", "name": "color/new",
        "type": "color", "values": {"light": "#000000", "dark": "#111111"}}})))
    .unwrap();
    s.apply_remote(&cmd(
        json!({"type": "removeMode", "collection": "t", "mode": "dark"}),
    ))
    .unwrap();
    s.apply_remote(&cmd(
        json!({"type": "addMode", "collection": "t", "mode": {"id": "dark", "name": "Dusk"}}),
    ))
    .unwrap();
    // The new `fg` and the new `dark` are not the ones I renamed: undo leaves them as they are.
    assert_eq!(s.undo_depth(), 0);
    assert!(s.undo().is_none());
    assert_eq!(
        (var_name(&s, "fg"), mode_name(&s, "t", "dark")),
        ("color/new".into(), "Dusk".into())
    );
}

#[test]
fn an_undo_that_the_names_of_the_collection_refuse_is_passed_and_does_not_block_the_stack() {
    let mut s = file();
    s.apply(&rename_variable("bg", "color/surface")).unwrap();
    // Another actor takes the old name: putting it back would be a duplicate.
    s.apply_remote(&cmd(
        json!({"type": "addVariable", "variable": {"id": "x", "collection": "t", "name": "color/bg",
        "type": "color", "values": {"light": "#000000", "dark": "#111111"}}}),
    ))
    .unwrap();
    assert!(
        s.undo().is_none(),
        "the rename back is refused as a duplicate name"
    );
    assert_eq!(var_name(&s, "bg"), "color/surface");
    // The step was passed, not left on the stack: the next undo does not try it again.
    assert_eq!((s.undo_depth(), s.redo_depth()), (0, 0));
}

#[test]
fn the_undo_of_a_removal_brings_back_the_name_another_actor_gave_it_while_the_step_was_undone() {
    let mut s = file();
    for c in [
        cmd(json!({"type": "removeVariable", "id": "fg"})),
        cmd(json!({"type": "removeMode", "collection": "t", "mode": "dark"})),
        cmd(json!({"type": "removeCollection", "id": "u"})),
    ] {
        s.apply(&c).unwrap();
    }
    // Undo all three: the entities are back, so the other actor can rename them, and my steps are on the redo stack.
    assert!(s.undo().is_some() && s.undo().is_some() && s.undo().is_some());
    s.apply_remote(&rename_variable("fg", "color/text"))
        .unwrap();
    s.apply_remote(&rename_mode("t", "dark", "Night")).unwrap();
    s.apply_remote(&rename_collection("u", "scratch")).unwrap();
    // Redo removes them again; undoing that adds them back as the other actor left them, not as they were.
    assert!(s.redo().is_some() && s.redo().is_some() && s.redo().is_some());
    assert!(s.vars().variable("fg").is_none() && s.vars().collection("u").is_none());
    assert!(s.undo().is_some() && s.undo().is_some() && s.undo().is_some());
    assert_eq!(
        (
            var_name(&s, "fg"),
            mode_name(&s, "t", "dark"),
            col_name(&s, "u")
        ),
        ("color/text".into(), "Night".into(), "scratch".into())
    );
}

#[test]
fn the_snapshot_my_undo_adds_again_takes_a_rename_of_the_confirmed_file() {
    // The remote rename applies to the file the server has, where my pending removals have not happened yet; my
    // view has them applied, so its undo adds the entities again and must bring them back under their new names.
    let confirmed = file();
    let mut view = file();
    for c in [
        cmd(json!({"type": "removeVariable", "id": "fg"})),
        cmd(json!({"type": "removeMode", "collection": "t", "mode": "dark"})),
        cmd(json!({"type": "removeCollection", "id": "u"})),
    ] {
        view.apply(&c).unwrap();
    }
    view.transform_remote(&rename_variable("fg", "color/text"), confirmed.doc());
    view.transform_remote(&rename_mode("t", "dark", "Night"), confirmed.doc());
    view.transform_remote(&rename_collection("u", "scratch"), confirmed.doc());
    assert!(view.undo().is_some() && view.undo().is_some() && view.undo().is_some());
    assert_eq!(
        (
            var_name(&view, "fg"),
            mode_name(&view, "t", "dark"),
            col_name(&view, "u")
        ),
        ("color/text".into(), "Night".into(), "scratch".into())
    );
}

#[test]
fn a_remote_add_of_what_my_undone_steps_made_drops_my_rename_redo() {
    // I add an entity, rename it and undo both: my redo steps are for an entity that is gone. Another actor then
    // adds one with the same id: it is a new incarnation, and my redo must neither add nor rename it.
    let mut s = file();
    for c in [
        cmd(
            json!({"type": "addVariable", "variable": {"id": "nv", "collection": "t", "name": "color/new",
            "type": "color", "values": {"light": "#000000", "dark": "#111111"}}}),
        ),
        rename_variable("nv", "color/mine"),
        cmd(
            json!({"type": "addCollection", "collection": {"id": "nc", "name": "mine1", "modes": [{"id": "a", "name": "a"}]}}),
        ),
        rename_collection("nc", "mine2"),
        cmd(json!({"type": "addMode", "collection": "t", "mode": {"id": "nm", "name": "mine3"}})),
        rename_mode("t", "nm", "mine4"),
    ] {
        s.apply(&c).unwrap();
    }
    for _ in 0..6 {
        assert!(s.undo().is_some());
    }
    assert!(s.vars().variable("nv").is_none() && s.vars().collection("nc").is_none());
    s.apply_remote(&cmd(json!({"type": "addVariable", "variable": {"id": "nv", "collection": "t", "name": "color/theirs",
        "type": "color", "values": {"light": "#000000", "dark": "#111111"}}})))
    .unwrap();
    s.apply_remote(&cmd(json!({"type": "addCollection", "collection": {"id": "nc", "name": "theirs1", "modes": [{"id": "a", "name": "a"}]}})))
    .unwrap();
    s.apply_remote(&cmd(
        json!({"type": "addMode", "collection": "t", "mode": {"id": "nm", "name": "theirs2"}}),
    ))
    .unwrap();
    while s.redo().is_some() {}
    assert_eq!(
        (
            var_name(&s, "nv"),
            col_name(&s, "nc"),
            mode_name(&s, "t", "nm")
        ),
        ("color/theirs".into(), "theirs1".into(), "theirs2".into())
    );
}

#[test]
fn a_remote_add_of_the_collection_my_undone_steps_made_drops_my_mode_rename_redo() {
    let mut s = file();
    for c in [
        cmd(
            json!({"type": "addCollection", "collection": {"id": "nc", "name": "mine", "modes": [{"id": "a", "name": "a"}]}}),
        ),
        rename_mode("nc", "a", "mine-a"),
    ] {
        s.apply(&c).unwrap();
    }
    assert!(s.undo().is_some() && s.undo().is_some());
    s.apply_remote(&cmd(json!({"type": "addCollection", "collection": {"id": "nc", "name": "theirs", "modes": [{"id": "a", "name": "theirs-a"}]}})))
    .unwrap();
    while s.redo().is_some() {}
    assert_eq!(
        (col_name(&s, "nc"), mode_name(&s, "nc", "a")),
        ("theirs".into(), "theirs-a".into())
    );
}
