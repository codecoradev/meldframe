//! E3.3: variable commands change the variables, and `invert` gives the commands that put them back exactly.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_protocol::{Command, Variables};
use meldframe_variables::{apply, invert};
use serde_json::json;

fn cmd(v: serde_json::Value) -> Command {
    serde_json::from_value(v).unwrap()
}

fn seed() -> Variables {
    let mut v = Variables::default();
    for c in [
        json!({"type": "addCollection", "collection": {"id": "t", "name": "theme",
            "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}}),
        json!({"type": "addVariable", "variable": {"id": "bg", "collection": "t", "name": "color/bg", "type": "color",
            "values": {"light": "#ffffff", "dark": "#000000"}}}),
    ] {
        apply(&mut v, &cmd(c));
    }
    v
}

/// Apply `c`, then its inverse: the variables are as they were.
fn round_trip(before: &Variables, c: &Command) -> Variables {
    let undo = invert(before, c);
    let mut v = before.clone();
    apply(&mut v, c);
    assert_ne!(&v, before, "the command changed nothing");
    undo.iter().for_each(|u| apply(&mut v, u));
    v
}

#[test]
fn every_variable_command_has_an_exact_inverse() {
    let s = seed();
    for c in [
        json!({"type": "addCollection", "collection": {"id": "c2", "name": "x", "modes": [{"id": "m", "name": "m"}]}}),
        json!({"type": "addMode", "collection": "t", "mode": {"id": "hc", "name": "contrast"}}),
        json!({"type": "addMode", "collection": "t", "mode": {"id": "hc", "name": "contrast"}, "index": 1, "copyFrom": "dark"}),
        json!({"type": "removeMode", "collection": "t", "mode": "dark"}),
        json!({"type": "addVariable", "variable": {"id": "fg", "collection": "t", "name": "color/fg", "type": "color",
            "values": {"light": "#000000", "dark": {"var": "bg"}}}}),
        json!({"type": "removeVariable", "id": "bg"}),
        json!({"type": "setVariableValue", "id": "bg", "mode": "dark", "value": "#111111"}),
    ] {
        let c = cmd(c);
        assert_eq!(round_trip(&s, &c), s, "{c:?}");
    }
    // A collection with no variable can be removed and brought back.
    let mut empty = s.clone();
    apply(
        &mut empty,
        &cmd(json!({"type": "removeVariable", "id": "bg"})),
    );
    assert_eq!(
        round_trip(&empty, &cmd(json!({"type": "removeCollection", "id": "t"}))),
        empty
    );
}

#[test]
fn a_new_mode_copies_the_values_of_the_default_mode_and_a_removed_mode_takes_its_values_along() {
    let mut v = seed();
    apply(
        &mut v,
        &cmd(
            json!({"type": "addMode", "collection": "t", "mode": {"id": "hc", "name": "contrast"}}),
        ),
    );
    assert_eq!(v.collections[0].modes.len(), 3);
    assert_eq!(v.variables[0].values["hc"], v.variables[0].values["light"]);
    apply(
        &mut v,
        &cmd(json!({"type": "removeMode", "collection": "t", "mode": "hc"})),
    );
    assert_eq!(v, seed());
}
