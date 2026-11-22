//! D8: the text of `export_variables` read by the import. The round trip is not exact (ADR-0020): the export names a
//! variable `--<collection>-<name>` and the import reads `--<name>`, and only a mode named like the dark mode of a root
//! rule is read as a mode. These tests pin what comes back, so the dialog's help text and the docs stay true.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_protocol::Variables;
use serde_json::json;

fn file() -> Variables {
    serde_json::from_value(json!({
        "collections": [
            {"id": "c1", "name": "appearance", "modes": [{"id": "m1", "name": "light"}, {"id": "m2", "name": "dark"}]},
            {"id": "c2", "name": "sizes", "modes": [{"id": "m3", "name": "Default"}, {"id": "m4", "name": "Compact"}]}
        ],
        "variables": [
            {"id": "v1", "collection": "c1", "name": "color/brand", "type": "color", "values": {"m1": "#112233", "m2": "#aabbcc"}},
            {"id": "v2", "collection": "c1", "name": "color/accent", "type": "color", "values": {"m1": {"var": "v1"}, "m2": "#000000"}},
            {"id": "v3", "collection": "c2", "name": "radius", "type": "number", "values": {"m3": 12, "m4": 8}}
        ]
    }))
    .unwrap()
}

#[test]
fn every_variable_and_alias_comes_back_but_under_the_name_with_the_collection_in_front() {
    let css = export::export_variables(&file()).unwrap().css;
    let i = run(&css);
    assert_eq!(i.report.variables, 3, "{:?}", i.report);
    assert_eq!(i.report.aliases, 1);
    let v = applied(&i.commands).vars;
    let names: Vec<&str> = v.variables.iter().map(|x| x.name.as_str()).collect();
    assert!(names.contains(&"appearance-color-brand"), "{names:?}");
    assert!(names.contains(&"sizes-radius"), "{names:?}");
    assert!(
        !names.contains(&"color/brand"),
        "the name did not round-trip: {names:?}"
    );
}

#[test]
fn a_dark_mode_comes_back_and_a_mode_with_another_name_does_not() {
    let css = export::export_variables(&file()).unwrap().css;
    let i = run(&css);
    let v = applied(&i.commands).vars;
    let root = v.collection("col-root").expect("the root collection");
    let modes: Vec<&str> = root.modes.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        modes,
        ["light", "dark"],
        "one collection, the modes light and dark"
    );
    // The Compact block of the export is not on the root and picks no mode, so the report says it was skipped, by name.
    let skipped: Vec<&variables_import::Entry> = i
        .report
        .entries
        .iter()
        .filter(|e| e.kind == variables_import::EntryKind::Skipped)
        .collect();
    assert_eq!(skipped.len(), 1, "{:?}", i.report);
    assert_eq!(skipped[0].code, "selector_skipped");
    assert_eq!(skipped[0].name, ":root[data-mode-sizes=\"compact\"]");
}
