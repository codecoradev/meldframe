//! What running each palette entry makes (issue E8.1.5): every entry id against the action it dispatches, the host switch or the
//! part the front end keeps, and the states in which there is nothing for it to act on.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::palette::*;
use meldframe_app_rules::palette::{Plan, entries, plan};
use serde_json::{Value, json};

/// A plan as JSON, so that the table below can say it: the action, or the host's part by name.
fn said(p: &Plan) -> Value {
    match p {
        Plan::Dispatch(a) => serde_json::to_value(a).unwrap(),
        Plan::SetLayoutOnCanvas(on) => json!({"layoutOnCanvas": on}),
        Plan::Host(z) => json!({"host": format!("{z:?}")}),
        Plan::Nothing => json!("nothing"),
    }
}

fn frame(preset: &str, x: f64, y: f64) -> Value {
    json!({"type": "createPreset", "preset": preset, "x": x, "y": y, "parent": null})
}

#[test]
fn every_entry_makes_what_the_button_or_the_key_for_it_makes() {
    let state = state(ON);
    let want: Vec<(&str, Value)> = vec![
        ("tool-select", json!({"type": "setTool", "tool": "select"})),
        ("tool-frame", json!({"type": "setTool", "tool": "frame"})),
        (
            "tool-rectangle",
            json!({"type": "setTool", "tool": "rectangle"}),
        ),
        (
            "tool-ellipse",
            json!({"type": "setTool", "tool": "ellipse"}),
        ),
        ("tool-text", json!({"type": "setTool", "tool": "text"})),
        ("undo", json!({"type": "undo"})),
        ("redo", json!({"type": "redo"})),
        ("delete", json!({"type": "delete", "ids": ["f1"]})),
        // Centred on (0, 0), the halves of an odd size rounded up as JavaScript rounds them.
        ("preset-desktop", frame("desktop", -720.0, -450.0)),
        ("preset-desktopHd", frame("desktopHd", -960.0, -540.0)),
        ("preset-tablet", frame("tablet", -384.0, -512.0)),
        ("preset-mobile", frame("mobile", -195.0, -422.0)),
        ("preset-phoneSmall", frame("phoneSmall", -187.0, -333.0)),
        ("preset-a4", frame("a4", -297.0, -421.0)),
        ("preset-letter", frame("letter", -306.0, -396.0)),
        ("zoom-in", json!({"host": "In"})),
        ("zoom-out", json!({"host": "Out"})),
        ("zoom-reset", json!({"host": "Reset"})),
        ("zoom-fit", json!({"host": "Fit"})),
        ("zoom-fit-selection", json!({"host": "FitSelection"})),
        ("auto-layout", json!({"layoutOnCanvas": true})),
        (
            "toggle-layers",
            json!({"type": "togglePanel", "panel": "layers"}),
        ),
        (
            "toggle-inspector",
            json!({"type": "togglePanel", "panel": "properties"}),
        ),
        ("export", json!({"type": "openExport"})),
        ("variant", json!({"type": "makeVariant", "id": "f1"})),
        (
            "shortcuts",
            json!({"type": "overlay", "op": {"type": "open", "overlay": "help"}}),
        ),
    ];
    let all = entries(&state, false, Some(false));
    assert_eq!(all.len(), want.len());
    for (e, (id, says)) in all.iter().zip(&want) {
        assert_eq!(e.id, *id);
        assert_eq!(said(&plan(e.run, &state, false, (0.0, 0.0))), *says, "{id}");
    }
}

#[test]
fn the_switch_turns_the_other_way_when_it_is_on() {
    let state = state(ON);
    let e = entries(&state, false, Some(true))
        .into_iter()
        .find(|e| e.id == "auto-layout")
        .unwrap();
    assert_eq!(
        said(&plan(e.run, &state, true, (0.0, 0.0))),
        json!({"layoutOnCanvas": false})
    );
}

#[test]
fn with_nothing_to_act_on_an_entry_makes_nothing() {
    let none = state(Flags {
        can_delete: false,
        can_export: false,
        ..ON
    });
    for e in entries(&none, false, None)
        .iter()
        .filter(|e| ["delete", "export", "variant"].contains(&e.id.as_str()))
    {
        assert_eq!(
            said(&plan(e.run, &none, false, (0.0, 0.0))),
            json!("nothing"),
            "{}",
            e.id
        );
    }
}
