//! The golden table of `golden_names.rs` for the fields that a file which loads cannot fill: a collection and a variable
//! name are lowercase paths (the core refuses anything else), so these fields are driven through the pure functions of
//! `meldframe-app-parts` that build them, over a `Variables` that was never validated (a file from a client that skipped
//! the check, an old file). The names come out safe to show all the same: the app layer does not rely on the file.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::CASES;
use meldframe_app_model::BindField;
use meldframe_app_parts::picker::picker_view;
use meldframe_app_parts::properties::selected_properties;
use meldframe_app_parts::variables_panel::variables_view;
use meldframe_protocol::{Node, Variables};
use meldframe_render::ModeChoice;
use serde_json::json;

fn nodes() -> Vec<Node> {
    serde_json::from_value(json!([
        {"id": "root", "type": "frame", "fill": {"var": "v1"}},
    ]))
    .unwrap()
}

/// Every collection, mode and variable named `text`; `v1` is an alias of `v2` in the second mode.
fn vars(text: &str) -> Variables {
    serde_json::from_value(json!({
        "collections": [{"id": "c1", "name": text,
            "modes": [{"id": "m1", "name": text}, {"id": "m2", "name": text}]}],
        "variables": [
            {"id": "v1", "collection": "c1", "name": text, "type": "color",
             "values": {"m1": "#ffffff", "m2": {"var": "v2"}}},
            {"id": "v2", "collection": "c1", "name": text, "type": "color",
             "values": {"m1": "#000000", "m2": "#111111"}},
        ],
    }))
    .unwrap()
}

#[test]
fn the_variables_panel_names_a_collection_a_mode_a_variable_and_an_alias_safely() {
    for c in CASES {
        let (vars, nodes, choice) = (vars(c.given), nodes(), ModeChoice::default());
        let panel = variables_view(&vars, &nodes, &choice, None, "");
        let col = &panel.collections[0];
        let what = c.what;
        assert_eq!(col.name, c.name, "collection: {what}");
        assert!(col.modes.iter().all(|m| m.name == c.name), "mode: {what}");
        assert_eq!(col.modes[0].id, "m1", "an id is never cleaned");
        let v1 = col.variables.iter().find(|v| v.id == "v1").expect("v1");
        assert_eq!(v1.name, c.name, "variable: {what}");
        assert_eq!(v1.values[1].text, c.name, "alias: {what}");
        assert_eq!(v1.values[1].alias.as_deref(), Some("v2"));
    }
}

#[test]
fn the_bind_picker_and_the_chip_of_a_binding_name_a_variable_safely() {
    for c in CASES {
        let (vars, nodes, choice) = (vars(c.given), nodes(), ModeChoice::default());
        let field = BindField::Fill;
        let pick = picker_view(&vars, &choice, &nodes, "root".into(), field, String::new());
        let what = c.what;
        assert_eq!(pick.rows.len(), 2);
        let all_safe = pick
            .rows
            .iter()
            .all(|r| r.name == c.name && r.collection == c.name);
        assert!(all_safe, "picker: {what}");
        let selection = ["root".to_string()];
        let p = selected_properties(&vars, &choice, &nodes, &selection, None).expect("properties");
        assert_eq!(p.bindings[0].name, c.name, "chip: {what}");
        assert!(!p.bindings[0].missing);
    }
}

#[test]
fn a_variable_that_is_gone_shows_its_id_in_the_chip() {
    let nodes = nodes();
    let choice = ModeChoice::default();
    let selection = ["root".to_string()];
    let none = Variables::default();
    let p = selected_properties(&none, &choice, &nodes, &selection, None).expect("properties");
    assert_eq!(
        (p.bindings[0].name.as_str(), p.bindings[0].missing),
        ("v1", true)
    );
}
