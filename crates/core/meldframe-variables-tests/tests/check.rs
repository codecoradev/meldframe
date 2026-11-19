//! E3.3: variable commands and bindings are validated at command time. Every refusal has its own code, and a
//! refused command leaves the file as it was (`check_command_with` never changes anything).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_check::{ApplyError, Limits, check_command_with, check_file_with};
use meldframe_protocol::{Command, Doc, Node, Variables};
use meldframe_variables::apply;
use serde_json::{Value, json};

fn cmd(v: Value) -> Command {
    serde_json::from_value(v).unwrap()
}

fn node(v: Value) -> Node {
    serde_json::from_value(v).unwrap()
}

/// `appearance` (light, dark) with the colour `bg` and the number `radius`, `base` with the colour `blue`; a rect `r`
/// whose fill is bound to `bg`.
fn doc() -> Doc {
    let mut d = Doc::of_nodes(vec![
        node(json!({"id": "r", "type": "rect", "pos": "a0", "fill": {"var": "bg"}})),
        node(json!({"id": "p", "type": "rect", "pos": "a1"})),
    ]);
    for c in [
        json!({"type": "addCollection", "collection": {"id": "appearance", "name": "appearance",
            "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}}),
        json!({"type": "addCollection", "collection": {"id": "base", "name": "base", "modes": [{"id": "m", "name": "m"}]}}),
        json!({"type": "addVariable", "variable": {"id": "bg", "collection": "appearance", "name": "color/bg", "type": "color",
            "values": {"light": "#ffffff", "dark": "#000000"}}}),
        json!({"type": "addVariable", "variable": {"id": "round", "collection": "appearance", "name": "radius/card", "type": "number",
            "values": {"light": 8, "dark": 12}}}),
        json!({"type": "addVariable", "variable": {"id": "blue", "collection": "base", "name": "color/blue", "type": "color",
            "values": {"m": "#2563eb"}}}),
    ] {
        apply(&mut d.vars, &cmd(c));
    }
    d
}

fn check(d: &Doc, c: Value) -> Result<(), ApplyError> {
    check_command_with(&d.nodes, &d.vars, &cmd(c), &Limits::default())
}

fn code(d: &Doc, c: Value) -> &'static str {
    check(d, c).expect_err("the command must be refused").code()
}

#[test]
fn the_good_commands_pass() {
    let d = doc();
    for c in [
        json!({"type": "updateNode", "id": "p", "patch": {"fill": {"var": "blue"}}}),
        json!({"type": "updateNode", "id": "p", "patch": {"radius": {"var": "round"}, "opacity": 0.5}}),
        json!({"type": "addNode", "node": {"id": "q", "type": "rect", "fill": {"var": "bg"}}}),
        json!({"type": "addVariable", "variable": {"id": "fg", "collection": "appearance", "name": "color/fg", "type": "color",
            "values": {"light": {"var": "blue"}, "dark": "#ffffff"}}}),
        json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": {"var": "blue"}}),
        json!({"type": "addMode", "collection": "appearance", "mode": {"id": "hc", "name": "contrast"}, "index": 1}),
        json!({"type": "removeMode", "collection": "appearance", "mode": "dark"}),
        json!({"type": "removeVariable", "id": "round"}),
    ] {
        assert_eq!(check(&d, c.clone()), Ok(()), "{c}");
    }
}

#[test]
fn a_binding_to_a_missing_variable_or_of_the_wrong_type_is_refused() {
    let d = doc();
    let patch = |f: Value| json!({"type": "updateNode", "id": "p", "patch": f});
    assert_eq!(
        code(&d, patch(json!({"fill": {"var": "nope"}}))),
        "E_VARIABLE_NOT_FOUND"
    );
    assert_eq!(
        code(&d, patch(json!({"fill": {"var": "round"}}))),
        "E_TYPE_MISMATCH"
    );
    assert_eq!(
        code(&d, patch(json!({"radius": {"var": "bg"}}))),
        "E_TYPE_MISMATCH"
    );
    assert_eq!(
        code(
            &d,
            json!({"type": "addNode", "node": {"id": "q", "type": "rect", "opacity": {"var": "nope"}}})
        ),
        "E_VARIABLE_NOT_FOUND"
    );
}

#[test]
fn a_literal_value_must_have_the_type_of_its_variable() {
    let d = doc();
    assert_eq!(
        code(
            &d,
            json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": 3})
        ),
        "E_TYPE_MISMATCH"
    );
    assert_eq!(
        code(
            &d,
            json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": "red"})
        ),
        "E_INVALID_COLOR"
    );
    assert_eq!(
        code(
            &d,
            json!({"type": "setVariableValue", "id": "round", "mode": "dark", "value": {"var": "bg"}})
        ),
        "E_TYPE_MISMATCH"
    );
    assert_eq!(
        code(
            &d,
            json!({"type": "setVariableValue", "id": "bg", "mode": "nope", "value": "#fff"})
        ),
        "E_MODE_NOT_FOUND"
    );
    assert_eq!(
        code(
            &d,
            json!({"type": "setVariableValue", "id": "nope", "mode": "dark", "value": "#fff"})
        ),
        "E_VARIABLE_NOT_FOUND"
    );
}

#[test]
fn an_alias_that_closes_a_cycle_or_points_at_nothing_is_refused() {
    let mut d = doc();
    let set = |id: &str, to: &str| json!({"type": "setVariableValue", "id": id, "mode": "light", "value": {"var": to}});
    assert_eq!(code(&d, set("bg", "bg")), "E_ALIAS_CYCLE");
    assert_eq!(code(&d, set("bg", "nope")), "E_VARIABLE_NOT_FOUND");
    apply(
        &mut d.vars,
        &cmd(
            json!({"type": "addVariable", "variable": {"id": "a", "collection": "base",
        "name": "color/a", "type": "color", "values": {"m": {"var": "bg"}}}}),
        ),
    );
    // `a` aliases `bg`, so `bg` may not alias `a` (in any mode).
    assert_eq!(code(&d, set("bg", "a")), "E_ALIAS_CYCLE");
}

#[test]
fn an_alias_chain_longer_than_the_limit_is_refused() {
    let mut d = doc();
    // v0 -> v1 -> ... -> v16 is 16 steps; one more is refused. Build from the far end, so that every alias
    // points at a variable that exists.
    for i in (0..=16).rev() {
        let value = if i == 16 {
            json!("#000000")
        } else {
            json!({"var": format!("v{}", i + 1)})
        };
        let add = cmd(
            json!({"type": "addVariable", "variable": {"id": format!("v{i}"), "collection": "base",
            "name": format!("chain/v{i}"), "type": "color", "values": {"m": value}}}),
        );
        assert_eq!(
            check_command_with(&d.nodes, &d.vars, &add, &Limits::default()),
            Ok(()),
            "v{i}"
        );
        apply(&mut d.vars, &add);
    }
    let one_more = json!({"type": "addVariable", "variable": {"id": "top", "collection": "base",
        "name": "chain/top", "type": "color", "values": {"m": {"var": "v0"}}}});
    assert_eq!(code(&d, one_more), "E_ALIAS_TOO_DEEP");
}

#[test]
fn a_variable_in_use_cannot_be_removed_and_a_collection_must_be_empty() {
    let mut d = doc();
    assert_eq!(
        code(&d, json!({"type": "removeVariable", "id": "bg"})),
        "E_VARIABLE_IN_USE"
    );
    apply(
        &mut d.vars,
        &cmd(
            json!({"type": "addVariable", "variable": {"id": "a", "collection": "base",
        "name": "color/a", "type": "color", "values": {"m": {"var": "blue"}}}}),
        ),
    );
    assert_eq!(
        code(&d, json!({"type": "removeVariable", "id": "blue"})),
        "E_VARIABLE_IN_USE"
    );
    assert_eq!(
        code(&d, json!({"type": "removeCollection", "id": "base"})),
        "E_COLLECTION_NOT_EMPTY"
    );
    assert_eq!(
        code(&d, json!({"type": "removeVariable", "id": "nope"})),
        "E_VARIABLE_NOT_FOUND"
    );
    assert_eq!(
        code(&d, json!({"type": "removeCollection", "id": "nope"})),
        "E_COLLECTION_NOT_FOUND"
    );
}

#[test]
fn ids_names_and_modes_are_checked() {
    let d = doc();
    let var = |v: Value| json!({"type": "addVariable", "variable": v});
    let ok = json!({"id": "x", "collection": "base", "name": "color/x", "type": "color", "values": {"m": "#fff"}});
    let with = |k: &str, v: Value| {
        let mut o = ok.clone();
        o[k] = v;
        var(o)
    };
    assert_eq!(code(&d, with("id", json!("bg"))), "E_DUPLICATE_ID");
    assert_eq!(code(&d, with("id", json!("bad id"))), "E_INVALID_ID");
    assert_eq!(
        code(&d, with("collection", json!("nope"))),
        "E_COLLECTION_NOT_FOUND"
    );
    assert_eq!(
        code(&d, with("name", json!("color/blue"))),
        "E_DUPLICATE_NAME"
    );
    assert_eq!(code(&d, with("name", json!("Color/X"))), "E_INVALID_NAME");
    assert_eq!(code(&d, with("name", json!("a//b"))), "E_INVALID_NAME");
    assert_eq!(code(&d, with("values", json!({}))), "E_INCOMPLETE_VALUES");
    assert_eq!(
        code(&d, with("values", json!({"m": "#fff", "other": "#fff"}))),
        "E_INCOMPLETE_VALUES"
    );
    // The same name in another collection is fine.
    let other = var(
        json!({"id": "x", "collection": "appearance", "name": "color/blue", "type": "color",
        "values": {"light": "#fff", "dark": "#000"}}),
    );
    assert_eq!(check(&d, other), Ok(()));
    let col = |v: Value| json!({"type": "addCollection", "collection": v});
    assert_eq!(
        code(&d, col(json!({"id": "c", "name": "c", "modes": []}))),
        "E_INVALID_MODES"
    );
    assert_eq!(
        code(
            &d,
            col(
                json!({"id": "c", "name": "c", "modes": [{"id": "a", "name": "a"}, {"id": "a", "name": "b"}]})
            )
        ),
        "E_INVALID_MODES"
    );
    assert_eq!(
        code(
            &d,
            col(json!({"id": "appearance", "name": "c", "modes": [{"id": "a", "name": "a"}]}))
        ),
        "E_DUPLICATE_ID"
    );
    let mode = |m: &str, idx: Value| json!({"type": "addMode", "collection": "appearance", "mode": {"id": m, "name": m}, "index": idx});
    assert_eq!(code(&d, mode("dark", json!(null))), "E_INVALID_MODES");
    assert_eq!(code(&d, mode("hc", json!(0))), "E_DEFAULT_MODE");
    assert_eq!(code(&d, mode("hc", json!(9))), "E_OUT_OF_RANGE");
    assert_eq!(
        code(
            &d,
            json!({"type": "addMode", "collection": "appearance", "mode": {"id": "hc", "name": "hc"}, "copyFrom": "nope"})
        ),
        "E_MODE_NOT_FOUND"
    );
    assert_eq!(
        code(
            &d,
            json!({"type": "removeMode", "collection": "appearance", "mode": "light"})
        ),
        "E_DEFAULT_MODE"
    );
    assert_eq!(
        code(
            &d,
            json!({"type": "removeMode", "collection": "appearance", "mode": "nope"})
        ),
        "E_MODE_NOT_FOUND"
    );
}

#[test]
fn a_stored_file_is_checked_as_a_whole() {
    let d = doc();
    let check = |nodes: &[Node], vars: &Variables| {
        check_file_with(nodes, vars, &Limits::default()).map_err(|e| e.code())
    };
    assert_eq!(check(&d.nodes, &d.vars), Ok(()));
    let mut broken: Variables = d.vars.clone();
    broken.variables[0].values.remove("dark");
    assert_eq!(check(&d.nodes, &broken), Err("E_INCOMPLETE_VALUES"));
    let mut cyclic = d.vars.clone();
    cyclic.variables[0]
        .values
        .insert("light".into(), meldframe_protocol::Bind::var("bg"));
    assert_eq!(check(&d.nodes, &cyclic), Err("E_ALIAS_CYCLE"));
    // A binding must name a variable of the file.
    assert_eq!(
        check(&d.nodes, &Variables::default()),
        Err("E_VARIABLE_NOT_FOUND")
    );
}

#[test]
fn a_command_that_holds_no_variables_refuses_a_variable_command() {
    let nodes = vec![];
    let c = cmd(
        json!({"type": "addCollection", "collection": {"id": "c", "name": "c", "modes": [{"id": "m", "name": "m"}]}}),
    );
    assert_eq!(
        meldframe_check::check_command(&nodes, &c, &Limits::default())
            .unwrap_err()
            .code(),
        "E_NO_VARIABLES"
    );
}

#[test]
fn the_in_use_error_names_a_few_users_not_all_of_them() {
    let mut d = doc();
    for i in 0..12 {
        d.nodes.push(node(
            json!({"id": format!("user-{i:02}"), "type": "rect", "pos": format!("b{i:02}"), "fill": {"var": "bg"}}),
        ));
    }
    let e = check(&d, json!({"type": "removeVariable", "id": "bg"})).expect_err("in use");
    assert_eq!(e.code(), "E_VARIABLE_IN_USE");
    let text = e.to_string();
    assert!(text.matches("user-").count() <= 5, "{text}");
    assert!(text.contains("and others"), "{text}");
}
