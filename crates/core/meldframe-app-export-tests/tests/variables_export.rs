//! D8: the Export variables dialog. Every variable of the file as CSS, built by the `export` feature from the person's own
//! view, kept while the dialog is open and built again only when the variables change. View state: no op, no undo step.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{Action, Event, Property};
use meldframe_protocol::Command;
use serde_json::json;

fn commands(v: serde_json::Value) -> Vec<Command> {
    serde_json::from_value(v).unwrap()
}

/// A collection `appearance` with a light and a dark mode and two variables, one an alias of the other.
fn theme() -> Vec<Command> {
    commands(json!([
        {"type": "addCollection", "collection": {"id": "c1", "name": "appearance", "modes": [{"id": "m1", "name": "light"}, {"id": "m2", "name": "dark"}]}},
        {"type": "addVariable", "variable": {"id": "v1", "collection": "c1", "name": "color/brand", "type": "color", "values": {"m1": "#112233", "m2": "#aabbcc"}}},
        {"type": "addVariable", "variable": {"id": "v2", "collection": "c1", "name": "color/accent", "type": "color", "values": {"m1": {"var": "v1"}, "m2": "#000000"}}}
    ]))
}

fn export_dialog(l: &Loopback) -> Option<meldframe_app::VariablesExportDialog> {
    l.app.state().variables_io.export.clone()
}

fn changed(l: &mut Loopback) -> usize {
    let n = l
        .events
        .iter()
        .filter(|e| matches!(e, Event::VariablesIoChanged {}))
        .count();
    l.events.clear();
    n
}

#[test]
fn opening_builds_every_variable_of_the_file_as_css() {
    let mut l = Loopback::live();
    l.send(Action::EditVariables { commands: theme() });
    assert!(export_dialog(&l).is_none());
    l.send(Action::OpenVariablesExport {});
    let d = export_dialog(&l).unwrap();
    assert_eq!(d.error, None);
    assert_eq!((d.collections, d.variables, d.blocks), (1, 2, 2));
    assert_eq!(
        d.css,
        ":root {\n  --appearance-color-brand: #112233;\n  --appearance-color-accent: var(--appearance-color-brand);\n}\n:root[data-mode-appearance=\"dark\"] {\n  --appearance-color-brand: #aabbcc;\n  --appearance-color-accent: #000000;\n}\n"
    );
    assert_eq!(d.bytes, Some(d.css.len()));
    assert_eq!(d.limit, 256 * 1024);
    assert!(d.warnings.is_empty());
}

#[test]
fn a_file_without_variables_opens_an_empty_dialog() {
    let mut l = Loopback::live();
    l.send(Action::OpenVariablesExport {});
    let d = export_dialog(&l).unwrap();
    assert_eq!((d.css.as_str(), d.variables, d.collections), ("", 0, 0));
}

#[test]
fn opening_and_closing_change_nothing_in_the_file_and_the_undo_history() {
    let mut l = Loopback::live();
    l.send(Action::EditVariables { commands: theme() });
    let (vars, seq, undo) = (l.app.vars().clone(), l.seq, l.app.state().history.can_undo);
    let fx = l.send(Action::OpenVariablesExport {});
    assert!(sent(&fx).is_empty(), "no op is sent");
    l.send(Action::CloseVariablesExport {});
    assert!(export_dialog(&l).is_none());
    assert_eq!((l.app.vars().clone(), l.seq), (vars, seq));
    assert_eq!(l.app.state().history.can_undo, undo);
    // One undo takes the theme out, not a step the dialog made.
    l.send(Action::Undo {});
    assert!(l.app.vars().variables.is_empty());
}

#[test]
fn it_follows_a_change_of_the_variables_made_here_or_by_someone_else() {
    let mut l = Loopback::live();
    l.send(Action::EditVariables { commands: theme() });
    l.send(Action::OpenVariablesExport {});
    l.send(Action::SetVariableValue {
        variable: "v1".into(),
        mode: "m1".into(),
        value: meldframe_app::VariableValue::Literal {
            text: "#445566".into(),
        },
    });
    assert!(
        export_dialog(&l)
            .unwrap()
            .css
            .contains("--appearance-color-brand: #445566;")
    );
    l.remote(
        "agent:other",
        "k1",
        1,
        commands(json!([
            {"type": "addVariable", "variable": {"id": "v3", "collection": "c1", "name": "color/muted", "type": "color", "values": {"m1": "#777777", "m2": "#888888"}}}
        ]))
        .remove(0),
    );
    let d = export_dialog(&l).unwrap();
    assert_eq!(d.variables, 3);
    assert!(
        d.css.contains("--appearance-color-muted: #777777;"),
        "{}",
        d.css
    );
}

#[test]
fn it_is_built_again_only_when_the_variables_change() {
    let mut l = Loopback::live();
    l.send(Action::EditVariables { commands: theme() });
    assert_eq!(
        l.app.variables_export_builds(),
        0,
        "nothing while it is closed"
    );
    l.send(Action::OpenVariablesExport {});
    assert_eq!(l.app.variables_export_builds(), 1);
    changed(&mut l);
    for action in [
        Action::Select { ids: ids(&["r1"]) },
        Action::SetVariablesQuery { query: "x".into() },
        Action::Tick { now_ms: 5 },
        // A node edit is a change of the document and not of the variables.
        Action::SetProperty {
            id: "root".into(),
            property: Property::Fill("#12ab34".into()),
        },
        Action::Move {
            id: "r1".into(),
            x: 5.0,
            y: 5.0,
        },
    ] {
        l.send(action.clone());
        assert_eq!(
            l.app.variables_export_builds(),
            1,
            "{action:?} built it again"
        );
    }
    assert_eq!(changed(&mut l), 0, "and nothing was announced");
    l.send(Action::EditVariables {
        commands: commands(
            json!([{"type": "renameVariable", "id": "v1", "name": "color/primary"}]),
        ),
    });
    assert_eq!(l.app.variables_export_builds(), 2);
    assert_eq!(changed(&mut l), 1);
    l.send(Action::CloseVariablesExport {});
    l.send(Action::EditVariables {
        commands: commands(json!([{"type": "renameVariable", "id": "v1", "name": "color/main"}])),
    });
    assert_eq!(l.app.variables_export_builds(), 2, "closed: not built");
}

/// A remote op on a node the file does not hold: the app cannot follow the server any more and halts.
fn diverge(l: &mut Loopback) {
    l.remote(
        "agent:other",
        "k1",
        1,
        Command::UpdateNode {
            id: "ghost".into(),
            patch: serde_json::from_value(json!({"x": 1.0})).unwrap(),
        },
    );
    assert_eq!(l.app.state().connection, meldframe_app::Connection::Halted);
}

#[test]
fn a_halted_app_exports_nothing_and_says_why() {
    let mut l = Loopback::live();
    l.send(Action::EditVariables { commands: theme() });
    l.send(Action::OpenVariablesExport {});
    assert!(export_dialog(&l).is_some());
    diverge(&mut l);
    assert!(export_dialog(&l).is_none(), "the open dialog closes");
    l.events.clear();
    l.send(Action::OpenVariablesExport {});
    assert_eq!(refused(&mut l), ["out_of_sync"]);
    assert!(export_dialog(&l).is_none());
}
