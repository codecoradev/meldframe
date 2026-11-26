//! The Export dialog while it is open (D9): it is built again only when what the export reads has changed, it follows every
//! way the document changes, it says each change of its state as an event, and a halted app exports nothing.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{Action, App, Event, ExportFormat, Property};
use meldframe_protocol::Node;
use serde_json::json;

fn export_changed(l: &mut Loopback) -> usize {
    l.events
        .drain(..)
        .filter(|e| matches!(e, Event::ExportChanged {}))
        .count()
}

#[test]
fn every_change_of_the_export_state_is_an_event_and_a_quiet_dispatch_is_none() {
    // The web reads the state again only after an event, so a change with none would never be drawn.
    let mut l = Loopback::live();
    export_changed(&mut l);
    select(&mut l, "root");
    assert_eq!(export_changed(&mut l), 1, "the target appeared");
    open(&mut l);
    assert_eq!(export_changed(&mut l), 1, "the dialog opened");
    l.send(Action::SetExportFormat {
        format: ExportFormat::Css,
    });
    assert_eq!(export_changed(&mut l), 1, "the format changed");
    l.send(Action::SetExportFormat {
        format: ExportFormat::Css,
    });
    assert_eq!(export_changed(&mut l), 0, "the same format is no change");
    l.send(Action::SetProperty {
        id: "root".into(),
        property: Property::Fill("#12ab34".into()),
    });
    assert_eq!(
        export_changed(&mut l),
        1,
        "the document changed under the open dialog"
    );
    l.send(Action::CloseExport {});
    assert_eq!(export_changed(&mut l), 1, "the dialog closed");
    l.send(Action::CloseExport {});
    assert_eq!(
        export_changed(&mut l),
        0,
        "closing a closed dialog is no change"
    );
    l.send(Action::Move {
        id: "r1".into(),
        x: 5.0,
        y: 5.0,
    });
    assert_eq!(
        export_changed(&mut l),
        0,
        "an edit with the dialog closed and the target the same"
    );
}

#[test]
fn the_sizes_are_what_each_format_saves_and_what_the_limit_counts() {
    let mut l = Loopback::live();
    select(&mut l, "root");
    open(&mut l);
    let t = l.app.state().export.dialog.clone().unwrap();
    // Tailwind: the document and the stylesheet are two files, and the limit counts both.
    assert_eq!(t.html_bytes, Some(t.html.len()));
    assert_eq!(t.css_bytes, Some(t.css.len()));
    assert!(t.css_bytes > Some(0));
    assert_eq!(
        t.counted_bytes,
        Some(t.html_bytes.unwrap() + t.css_bytes.unwrap())
    );
    l.send(Action::SetExportFormat {
        format: ExportFormat::Css,
    });
    let c = l.app.state().export.dialog.clone().unwrap();
    // Plain CSS: one file with the stylesheet inside. The export carries the stylesheet a second time for the caller,
    // and the limit counts it, but nothing of it is saved or copied.
    assert_eq!((c.html_bytes, c.css_bytes), (Some(c.html.len()), Some(0)));
    assert!(c.css.is_empty());
    assert!(
        c.counted_bytes > c.html_bytes,
        "the limit counts the stylesheet twice in this form"
    );
}

fn builds_after(l: &mut Loopback, action: Action) -> (u64, u64) {
    let before = l.app.export_builds();
    l.send(action);
    (before, l.app.export_builds())
}

#[test]
fn an_open_dialog_is_not_built_again_by_what_does_not_change_the_export() {
    let mut l = Loopback::live();
    select(&mut l, "root");
    open(&mut l);
    for action in [
        Action::Select { ids: ids(&["r1"]) },
        Action::Select {
            ids: ids(&["root"]),
        },
        Action::SetTool {
            tool: meldframe_app::Tool::Rectangle,
        },
        Action::TogglePanel {
            panel: meldframe_app::Panel::Layers,
        },
        Action::SetVariablesQuery { query: "x".into() },
        Action::Tick { now_ms: 5 },
    ] {
        let (before, after) = builds_after(&mut l, action.clone());
        assert_eq!(before, after, "{action:?} rebuilt the export");
    }
    // The same format again changes nothing, and neither does another dispatch of the same view.
    let (before, after) = builds_after(
        &mut l,
        Action::SetExportFormat {
            format: ExportFormat::Tailwind,
        },
    );
    assert_eq!(before, after);
    // What does change it builds it once: an edit, and the other format.
    let (before, after) = builds_after(
        &mut l,
        Action::SetProperty {
            id: "root".into(),
            property: Property::Fill("#12ab34".into()),
        },
    );
    assert_eq!(after, before + 1);
    let (before, after) = builds_after(
        &mut l,
        Action::SetExportFormat {
            format: ExportFormat::Css,
        },
    );
    assert_eq!(after, before + 1);
}

fn server_event(seq: u64, actor: &str, command: serde_json::Value) -> Action {
    Action::Frame {
        now_ms: 0,
        frame: meldframe_app::Frame::Event {
            seq,
            actor: actor.into(),
            key: None,
            command: serde_json::from_value(command).unwrap(),
        },
    }
}

#[test]
fn the_dialog_follows_every_way_the_document_changes() {
    let html = |l: &Loopback| l.app.state().export.dialog.clone().unwrap().html;
    let mut l = Loopback::live();
    select(&mut l, "root");
    open(&mut l);
    let first = html(&l);
    // An agent's change, as the server sends it.
    l.send(server_event(
        1,
        "agent:a",
        json!({"type": "updateNode", "id": "root", "patch": {"fill": "#aa00bb"}}),
    ));
    l.seq = 1; // the loopback numbers the person's ops after the agent's
    let remote = html(&l);
    assert_ne!(first, remote);
    assert!(remote.contains("#aa00bb"));
    // The person's own edit, and its undo.
    l.send(Action::SetProperty {
        id: "root".into(),
        property: Property::Fill("#12ab34".into()),
    });
    let local = html(&l);
    assert!(local.contains("#12ab34"));
    l.send(Action::Undo {});
    assert_eq!(html(&l), remote);
    // A new snapshot of the file.
    let mut nodes = doc();
    nodes[0] = serde_json::from_value(
        json!({"id": "root", "type": "frame", "parentId": null, "fill": "#0f0f0f"}),
    )
    .unwrap();
    l.send(Action::Reload {
        now_ms: 1,
        nodes,
        variables: Default::default(),
        seq: 9,
        applied: vec![],
    });
    assert!(html(&l).contains("#0f0f0f"));
}

#[test]
fn the_dialog_follows_a_change_of_a_variable() {
    let nodes: Vec<Node> = serde_json::from_value(json!([{"id": "root", "type": "frame", "parentId": null, "w": 200, "h": 100, "fill": {"var": "bg"}}])).unwrap();
    let vars = serde_json::from_value(json!({
        "collections": [{"id": "t", "name": "appearance", "modes": [{"id": "light", "name": "light"}]}],
        "variables": [{"id": "bg", "collection": "t", "name": "color/bg", "type": "color", "values": {"light": "#ffffff"}}],
    }))
    .unwrap();
    let mut app = App::with_doc(
        "user:me",
        "c0",
        1,
        meldframe_protocol::Doc { nodes, vars },
        0,
    )
    .unwrap();
    app.dispatch(Action::Select {
        ids: ids(&["root"]),
    });
    app.dispatch(Action::OpenExport {});
    let before = app.state().export.dialog.clone().unwrap().css;
    app.dispatch(Action::SetVariableValue {
        variable: "bg".into(),
        mode: "light".into(),
        value: meldframe_app::VariableValue::Literal {
            text: "#445566".into(),
        },
    });
    let after = app.state().export.dialog.clone().unwrap().css;
    assert_ne!(before, after);
    assert!(after.contains("#445566"));
}

fn diverge(l: &mut Loopback) {
    l.send(server_event(
        1,
        "agent:a",
        json!({"type": "updateNode", "id": "ghost", "patch": {"x": 1.0}}),
    ));
    assert_eq!(l.app.state().connection, meldframe_app::Connection::Halted);
}

#[test]
fn a_page_out_of_step_with_the_server_exports_nothing() {
    // The view of a halted app may be out of date: it cannot be exported, and an open dialog closes.
    let mut l = Loopback::live();
    select(&mut l, "root");
    open(&mut l);
    assert!(l.app.state().export.dialog.is_some());
    diverge(&mut l);
    assert!(
        l.app.state().export.dialog.is_none(),
        "the dialog closes when the app halts"
    );
    refused(&mut l);
    open(&mut l);
    assert_eq!(refused(&mut l), vec!["out_of_sync".to_string()]);
    assert!(l.app.state().export.dialog.is_none());
}
