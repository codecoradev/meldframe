//! The rename commands (D8): the wire shape, what is refused and with which code, apply, the exact inverse, the
//! footprint and the names the rest of the system knows them by. A rename changes a name and nothing that points
//! at the entity: ids are the identity.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-variables-tests/tests/common/mod.rs"]
mod common;

use common::{add, appearance, bg, bind_fill, cmd};
use meldframe_check::{Limits, check_command_with};
use meldframe_core::apply_doc;
use meldframe_protocol::{Command, Doc, Variables};
use meldframe_variables::{ModeChoice, check_variable_command};
use meldframe_variables::{apply, footprint, invert, is_variable_command, resolve, touched};
use serde_json::{Value, json};

fn rename_collection(id: &str, name: &str) -> Command {
    cmd(json!({"type": "renameCollection", "id": id, "name": name}))
}

fn rename_mode(collection: &str, mode: &str, name: &str) -> Command {
    cmd(json!({"type": "renameMode", "collection": collection, "mode": mode, "name": name}))
}

fn rename_variable(id: &str, name: &str) -> Command {
    cmd(json!({"type": "renameVariable", "id": id, "name": name}))
}

/// Collection `t` (modes light, dark), colour `bg` ("color/bg"), a second colour `fg` ("color/fg") in `t`, and a
/// second collection `u` with a variable that is also named "color/bg".
fn file() -> Doc {
    let mut doc = Doc::default();
    let l = Limits::default();
    for c in [
        appearance(),
        bg(),
        cmd(
            json!({"type": "addVariable", "variable": {"id": "fg", "collection": "t", "name": "color/fg",
            "type": "color", "values": {"light": "#000000", "dark": {"var": "bg"}}}}),
        ),
        cmd(
            json!({"type": "addCollection", "collection": {"id": "u", "name": "other", "modes": [{"id": "m", "name": "m"}]}}),
        ),
        cmd(
            json!({"type": "addVariable", "variable": {"id": "ubg", "collection": "u", "name": "color/bg",
            "type": "color", "values": {"m": "#123456"}}}),
        ),
        add("card", "rect", None),
        bind_fill("card", "bg"),
    ] {
        apply_doc(&mut doc, &c, &l).unwrap();
    }
    doc
}

fn code(doc: &Doc, c: &Command) -> Option<&'static str> {
    check_command_with(&doc.nodes, &doc.vars, c, &Limits::default())
        .err()
        .map(|e| e.code())
}

fn applied(doc: &Doc, c: &Command) -> Variables {
    let mut vars = doc.vars.clone();
    apply(&mut vars, c);
    vars
}

#[test]
fn the_three_commands_have_the_wire_shape_of_the_other_variable_commands() {
    for (c, want) in [
        (
            rename_collection("t", "theme"),
            json!({"type": "renameCollection", "id": "t", "name": "theme"}),
        ),
        (
            rename_mode("t", "dark", "Night"),
            json!({"type": "renameMode", "collection": "t", "mode": "dark", "name": "Night"}),
        ),
        (
            rename_variable("bg", "color/surface"),
            json!({"type": "renameVariable", "id": "bg", "name": "color/surface"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&c).unwrap(), want);
        assert!(is_variable_command(&c));
    }
    // Unknown fields and missing ones are refused, as for every other command.
    for bad in [
        json!({"type": "renameVariable", "id": "bg", "name": "x", "to": "y"}),
        json!({"type": "renameVariable", "id": "bg"}),
        json!({"type": "renameMode", "collection": "t", "name": "x"}),
        json!({"type": "renameCollection", "name": "x"}),
    ] {
        assert!(serde_json::from_value::<Command>(bad).is_err());
    }
}

#[test]
fn a_node_only_caller_refuses_them_like_any_variable_command() {
    let doc = file();
    for c in [
        rename_collection("t", "theme"),
        rename_mode("t", "dark", "Night"),
        rename_variable("bg", "color/surface"),
    ] {
        assert_eq!(
            meldframe_check::check_command(&doc.nodes, &c, &Limits::default())
                .unwrap_err()
                .code(),
            "E_NO_VARIABLES"
        );
    }
}

#[test]
fn rename_collection_checks_the_collection_and_the_name() {
    let doc = file();
    assert_eq!(code(&doc, &rename_collection("t", "theme/dark-ui")), None);
    assert_eq!(
        code(&doc, &rename_collection("gone", "theme")),
        Some("E_COLLECTION_NOT_FOUND")
    );
    for bad in ["Theme", "a b", "", "theme/", "-x", "a--b"] {
        assert_eq!(
            code(&doc, &rename_collection("t", bad)),
            Some("E_INVALID_NAME"),
            "{bad:?}"
        );
    }
    let long = "a".repeat(Limits::default().max_name + 1);
    assert_eq!(
        code(&doc, &rename_collection("t", &long)),
        Some("E_TOO_LONG")
    );
    // A collection name is not unique, as `addCollection` does not make it (a decision of D8).
    assert_eq!(code(&doc, &rename_collection("t", "other")), None);
}

#[test]
fn rename_mode_applies_the_rule_of_add_mode() {
    let doc = file();
    assert_eq!(code(&doc, &rename_mode("t", "dark", "Night")), None);
    // `addMode` takes any name up to the length limit, so a rename does too.
    assert_eq!(code(&doc, &rename_mode("t", "dark", "High Contrast")), None);
    assert_eq!(
        code(&doc, &rename_mode("gone", "dark", "x")),
        Some("E_COLLECTION_NOT_FOUND")
    );
    assert_eq!(
        code(&doc, &rename_mode("t", "gone", "x")),
        Some("E_MODE_NOT_FOUND")
    );
    // The mode must be one of that collection: `m` belongs to `u`.
    assert_eq!(
        code(&doc, &rename_mode("t", "m", "x")),
        Some("E_MODE_NOT_FOUND")
    );
    let long = "a".repeat(Limits::default().max_name + 1);
    assert_eq!(
        code(&doc, &rename_mode("t", "dark", &long)),
        Some("E_TOO_LONG")
    );
    // The default mode may be renamed (only its removal and its place are protected).
    assert_eq!(code(&doc, &rename_mode("t", "light", "Day")), None);
}

#[test]
fn rename_variable_checks_the_variable_the_name_and_the_names_of_its_collection() {
    let doc = file();
    assert_eq!(
        code(&doc, &rename_variable("bg", "color/surface/500")),
        None
    );
    assert_eq!(
        code(&doc, &rename_variable("gone", "color/x")),
        Some("E_VARIABLE_NOT_FOUND")
    );
    for bad in ["Color/Bg", "color//bg", "color bg", "", "/x"] {
        assert_eq!(
            code(&doc, &rename_variable("bg", bad)),
            Some("E_INVALID_NAME"),
            "{bad:?}"
        );
    }
    // Another variable of the same collection has the name.
    assert_eq!(
        code(&doc, &rename_variable("bg", "color/fg")),
        Some("E_DUPLICATE_NAME")
    );
    // The same name in another collection is fine: `ubg` is called "color/bg" in `u`.
    assert_eq!(code(&doc, &rename_variable("fg", "color/other")), None);
    assert_eq!(code(&doc, &rename_variable("ubg", "color/fg")), None);
}

#[test]
fn a_name_of_exactly_the_longest_length_is_accepted() {
    let doc = file();
    let max = Limits::default().max_name;
    let exact = "a".repeat(max);
    for c in [
        rename_collection("t", &exact),
        rename_mode("t", "dark", &exact),
        rename_variable("bg", &exact),
    ] {
        assert_eq!(code(&doc, &c), None, "{c:?}");
    }
    // One more is refused, with the code of the rule each name follows.
    let over = "a".repeat(max + 1);
    assert_eq!(
        code(&doc, &rename_variable("bg", &over)),
        Some("E_TOO_LONG")
    );
}

#[test]
fn a_rename_to_the_current_name_is_accepted_and_changes_nothing() {
    let doc = file();
    for c in [
        rename_collection("t", "appearance"),
        rename_mode("t", "dark", "dark"),
        rename_variable("bg", "color/bg"),
    ] {
        assert_eq!(code(&doc, &c), None, "{c:?}");
        assert_eq!(applied(&doc, &c), doc.vars);
    }
    // ... and that is not because a rename does nothing.
    assert_ne!(applied(&doc, &rename_variable("bg", "color/x")), doc.vars);
}

#[test]
fn apply_changes_the_name_and_nothing_else() {
    let doc = file();
    let v = applied(&doc, &rename_variable("bg", "color/surface"));
    assert_eq!(v.variable("bg").unwrap().name, "color/surface");
    let mut want = doc.vars.clone();
    want.variables[0].name = "color/surface".into();
    assert_eq!(v, want);

    let v = applied(&doc, &rename_collection("t", "theme"));
    assert_eq!(v.collection("t").unwrap().name, "theme");
    assert_eq!(
        v.collection("t").unwrap().modes,
        doc.vars.collection("t").unwrap().modes
    );

    let v = applied(&doc, &rename_mode("t", "dark", "Night"));
    let modes = &v.collection("t").unwrap().modes;
    assert_eq!(
        (
            modes[1].id.as_str(),
            modes[1].name.as_str(),
            modes[0].name.as_str()
        ),
        ("dark", "Night", "light")
    );
    // The values keep their mode id: nothing is copied, moved or dropped.
    assert_eq!(v.variables, doc.vars.variables);
}

#[test]
fn a_binding_and_an_alias_survive_a_rename_of_what_they_point_at() {
    let mut doc = file();
    let l = Limits::default();
    let before = resolve(&doc.vars, "fg", &ModeChoice::of([("t", "dark")]));
    for c in [
        rename_variable("bg", "color/surface"),
        rename_mode("t", "dark", "Night"),
        rename_collection("t", "theme"),
    ] {
        apply_doc(&mut doc, &c, &l).unwrap();
    }
    assert_eq!(doc.vars.variable("bg").unwrap().name, "color/surface");
    // The node is still bound to `bg` and `fg` still aliases it, by id.
    assert_eq!(doc.nodes[0].fill, meldframe_protocol::Bind::var("bg"));
    assert_eq!(
        doc.vars.variable("fg").unwrap().values["dark"],
        meldframe_protocol::Bind::var("bg")
    );
    assert_eq!(
        resolve(&doc.vars, "fg", &ModeChoice::of([("t", "dark")])),
        before
    );
    // And a variable that is bound can be renamed although `removeVariable` would be refused.
    assert_eq!(
        code(&doc, &Command::RemoveVariable { id: "bg".into() }),
        Some("E_VARIABLE_IN_USE")
    );
}

#[test]
fn the_inverse_of_a_rename_is_the_rename_back_and_is_exact() {
    let doc = file();
    for c in [
        rename_collection("t", "theme"),
        rename_mode("t", "dark", "Night"),
        rename_variable("bg", "color/surface"),
        // A no-op rename inverts to itself.
        rename_variable("bg", "color/bg"),
    ] {
        let inv = invert(&doc.vars, &c);
        assert_eq!(inv.len(), 1, "{c:?}");
        // The inverse is itself a valid command on the result, and applying it restores the variables.
        let mut after = doc.clone();
        apply_doc(&mut after, &c, &Limits::default()).unwrap();
        apply_doc(&mut after, &inv[0], &Limits::default()).unwrap();
        assert_eq!(after.vars, doc.vars, "{c:?}");
    }
    assert_eq!(
        serde_json::to_value(invert(&doc.vars, &rename_variable("bg", "color/surface"))).unwrap(),
        json!([{"type": "renameVariable", "id": "bg", "name": "color/bg"}])
    );
    assert_eq!(
        serde_json::to_value(invert(&doc.vars, &rename_mode("t", "dark", "Night"))).unwrap(),
        json!([{"type": "renameMode", "collection": "t", "mode": "dark", "name": "dark"}])
    );
    assert_eq!(
        serde_json::to_value(invert(&doc.vars, &rename_collection("t", "theme"))).unwrap(),
        json!([{"type": "renameCollection", "id": "t", "name": "appearance"}])
    );
}

#[test]
fn if_seq_names_the_entity_the_rename_is_about() {
    assert_eq!(touched(&rename_variable("bg", "x")), vec!["$var:bg"]);
    assert_eq!(touched(&rename_collection("t", "x")), vec!["$col:t"]);
    assert_eq!(touched(&rename_mode("t", "dark", "x")), vec!["$col:t"]);
}

const MARKS: (&str, &str, &str) = ("exists", "all", "whole");

fn writes(c: &Command) -> Vec<(String, String)> {
    footprint(c, MARKS).1
}

#[test]
fn the_footprint_of_a_rename_is_the_name_of_the_entity_and_not_its_values() {
    let name_of_bg = writes(&rename_variable("bg", "x"));
    assert_eq!(name_of_bg.len(), 1);
    assert_eq!(name_of_bg[0].0, "$var:bg");
    // Two renames of one entity write the same field; a rename and a value write different ones.
    assert_eq!(name_of_bg, writes(&rename_variable("bg", "y")));
    for mode in ["light", "dark", "name"] {
        let set =
            cmd(json!({"type": "setVariableValue", "id": "bg", "mode": mode, "value": "#000000"}));
        assert!(
            writes(&set).iter().all(|w| !name_of_bg.contains(w)),
            "a value of mode {mode:?} is not the name"
        );
    }
    assert_ne!(name_of_bg, writes(&rename_variable("fg", "x")));
    let (reads, _) = footprint(&rename_variable("bg", "x"), MARKS);
    assert!(reads.contains(&("$var:bg".into(), "exists".into())));
    assert!(reads.iter().any(|r| name_of_bg.contains(r)));

    let col = writes(&rename_collection("t", "x"));
    assert_eq!((col.len(), col[0].0.as_str()), (1, "$col:t"));
    // The name of a collection is not the name of one of its modes, nor a mode that happens to be called `name`.
    let m = writes(&rename_mode("t", "dark", "x"));
    assert_eq!((m.len(), m[0].0.as_str()), (1, "$col:t"));
    assert_ne!(col, m);
    assert_ne!(m, writes(&rename_mode("t", "light", "x")));
    let add_name_mode =
        cmd(json!({"type": "addMode", "collection": "t", "mode": {"id": "name", "name": "n"}}));
    assert!(writes(&add_name_mode).iter().all(|w| !col.contains(w)));
    // A rename of a mode reads that the mode exists, so removing the mode is a premise of it.
    let (reads, _) = footprint(&rename_mode("t", "dark", "x"), MARKS);
    assert!(reads.contains(&("$col:t".into(), "dark".into())));
    // A mode that is added again brings its name: the add writes what the rename writes.
    let add_dark =
        cmd(json!({"type": "addMode", "collection": "t", "mode": {"id": "dark", "name": "d"}}));
    assert!(writes(&add_dark).iter().any(|w| m.contains(w)));
}

#[test]
fn the_check_is_the_same_whether_it_runs_alone_or_through_the_variable_crate() {
    // `check_variable_command` is what `meldframe-check` calls; a rename of a missing variable is refused there too.
    let doc = file();
    let e = check_variable_command(
        &doc.nodes,
        &doc.vars,
        &rename_variable("gone", "a"),
        &Limits::default().var_limits(),
    );
    assert_eq!(e.unwrap_err().code(), "E_VARIABLE_NOT_FOUND");
}

#[test]
fn a_command_name_is_what_a_client_sends() {
    assert_eq!(
        [
            meldframe_batch::command_name(&rename_collection("t", "x")),
            meldframe_batch::command_name(&rename_mode("t", "m", "x")),
            meldframe_batch::command_name(&rename_variable("v", "x")),
        ],
        ["renameCollection", "renameMode", "renameVariable"]
    );
    let _: Value = json!(null);
}
