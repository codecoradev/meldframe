//! E3.3: resolving a variable through aliases and modes. The mode a viewer picks changes the answer, never the
//! file; a broken chain answers `None` and never panics.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_protocol::{Bind, Value, Variables};
use meldframe_variables::{MAX_ALIAS_DEPTH, ModeChoice, resolve};
use serde_json::json;

fn vars(v: serde_json::Value) -> Variables {
    serde_json::from_value(v).unwrap()
}

/// `theme` has light and dark; `brand` aliases `blue` of the one-mode collection `base`.
fn design_system() -> Variables {
    vars(json!({
        "collections": [
            {"id": "base", "name": "base", "modes": [{"id": "m0", "name": "default"}]},
            {"id": "theme", "name": "theme", "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}
        ],
        "variables": [
            {"id": "blue", "collection": "base", "name": "color/blue", "type": "color", "values": {"m0": "#2563eb"}},
            {"id": "sky", "collection": "base", "name": "color/sky", "type": "color", "values": {"m0": "#38bdf8"}},
            {"id": "brand", "collection": "theme", "name": "color/brand", "type": "color",
             "values": {"light": {"var": "blue"}, "dark": {"var": "sky"}}},
            {"id": "bg", "collection": "theme", "name": "color/bg", "type": "color",
             "values": {"light": "#ffffff", "dark": "#0a0a0a"}}
        ]
    }))
}

#[test]
fn a_literal_resolves_in_the_default_mode_and_an_alias_follows_the_chosen_mode() {
    let v = design_system();
    let none = ModeChoice::default();
    assert_eq!(
        resolve(&v, "bg", &none),
        Some(Value::Color("#ffffff".into()))
    );
    assert_eq!(
        resolve(&v, "brand", &none),
        Some(Value::Color("#2563eb".into()))
    );
    let dark = ModeChoice::of([("theme", "dark")]);
    assert_eq!(
        resolve(&v, "bg", &dark),
        Some(Value::Color("#0a0a0a".into()))
    );
    assert_eq!(
        resolve(&v, "brand", &dark),
        Some(Value::Color("#38bdf8".into()))
    );
}

#[test]
fn an_unknown_mode_falls_back_to_the_default_mode() {
    let v = design_system();
    let bad = ModeChoice::of([("theme", "no-such-mode")]);
    assert_eq!(
        resolve(&v, "bg", &bad),
        Some(Value::Color("#ffffff".into()))
    );
}

#[test]
fn a_missing_variable_resolves_to_none() {
    assert_eq!(
        resolve(&design_system(), "nope", &ModeChoice::default()),
        None
    );
}

#[test]
fn a_cycle_or_a_chain_that_is_too_long_resolves_to_none() {
    let mut v = design_system();
    // Corrupt data that the checks would refuse: blue points at brand, which points at blue.
    v.variables[0]
        .values
        .insert("m0".into(), Bind::var("brand"));
    assert_eq!(resolve(&v, "brand", &ModeChoice::default()), None);

    let mut chain = Variables {
        collections: design_system().collections[..1].to_vec(),
        ..Default::default()
    };
    for i in 0..=MAX_ALIAS_DEPTH + 1 {
        let value = if i == MAX_ALIAS_DEPTH + 1 {
            Bind::Lit(Value::Number(1.0))
        } else {
            Bind::var(format!("v{}", i + 1))
        };
        chain.variables.push(serde_json::from_value(json!({
            "id": format!("v{i}"), "collection": "base", "name": format!("n{i}"), "type": "number", "values": {}
        })).map(|mut x: meldframe_protocol::Variable| { x.values.insert("m0".into(), value); x }).unwrap());
    }
    assert_eq!(resolve(&chain, "v0", &ModeChoice::default()), None);
    assert!(resolve(&chain, "v2", &ModeChoice::default()).is_some());
}
