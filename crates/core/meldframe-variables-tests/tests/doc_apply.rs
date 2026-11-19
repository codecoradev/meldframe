//! E3.3: variable commands and bindings go through the same `apply` as node commands. A refused command
//! changes neither the nodes nor the variables.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_core::{Limits, apply_doc, plan_doc};
use meldframe_protocol::{Bind, Command, Doc};
use serde_json::{Value, json};

fn cmd(v: Value) -> Command {
    serde_json::from_value(v).unwrap()
}

fn run(d: &mut Doc, v: Value) -> Result<(), String> {
    apply_doc(d, &cmd(v), &Limits::default()).map_err(|e| e.code().to_string())
}

fn with_theme() -> Doc {
    let mut d = Doc::default();
    for c in [
        json!({"type": "addCollection", "collection": {"id": "theme", "name": "theme",
            "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}}),
        json!({"type": "addVariable", "variable": {"id": "bg", "collection": "theme", "name": "color/bg", "type": "color",
            "values": {"light": "#ffffff", "dark": "#000000"}}}),
        json!({"type": "addNode", "node": {"id": "card", "type": "rect"}}),
    ] {
        run(&mut d, c).unwrap();
    }
    d
}

#[test]
fn a_node_binds_a_variable_and_the_binding_is_part_of_the_document() {
    let mut d = with_theme();
    run(
        &mut d,
        json!({"type": "updateNode", "id": "card", "patch": {"fill": {"var": "bg"}}}),
    )
    .unwrap();
    assert_eq!(d.nodes[0].fill, Bind::var("bg"));
    // The document keeps the binding, not a resolved colour: the JSON round-trips.
    let n = serde_json::to_value(&d.nodes[0]).unwrap();
    assert_eq!(n["fill"], json!({"var": "bg"}));
}

#[test]
fn a_refused_command_changes_nothing() {
    let mut d = with_theme();
    run(
        &mut d,
        json!({"type": "updateNode", "id": "card", "patch": {"fill": {"var": "bg"}}}),
    )
    .unwrap();
    let before = (serde_json::to_string(&d.nodes).unwrap(), d.vars.clone());
    for c in [
        json!({"type": "removeVariable", "id": "bg"}),
        json!({"type": "updateNode", "id": "card", "patch": {"fill": {"var": "gone"}}}),
        json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": {"var": "bg"}}),
    ] {
        assert!(run(&mut d, c).is_err());
        assert_eq!(
            (serde_json::to_string(&d.nodes).unwrap(), d.vars.clone()),
            before
        );
    }
}

#[test]
fn removing_a_node_frees_its_variable() {
    let mut d = with_theme();
    run(
        &mut d,
        json!({"type": "updateNode", "id": "card", "patch": {"radius": {"var": "r"}}}),
    )
    .unwrap_err();
    run(
        &mut d,
        json!({"type": "updateNode", "id": "card", "patch": {"fill": {"var": "bg"}}}),
    )
    .unwrap();
    assert_eq!(
        run(&mut d, json!({"type": "removeVariable", "id": "bg"})),
        Err("E_VARIABLE_IN_USE".into())
    );
    run(&mut d, json!({"type": "removeNode", "id": "card"})).unwrap();
    run(&mut d, json!({"type": "removeVariable", "id": "bg"})).unwrap();
    run(&mut d, json!({"type": "removeCollection", "id": "theme"})).unwrap();
    assert!(d.vars.is_empty());
}

#[test]
fn plan_does_not_change_the_document() {
    let d = with_theme();
    let c =
        cmd(json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": "#111111"}));
    assert_eq!(plan_doc(&d, &c, &Limits::default()), Ok(None));
    assert_eq!(
        d.vars.variables[0].values["dark"],
        Bind::Lit(meldframe_protocol::Value::Color("#000000".into()))
    );
}
