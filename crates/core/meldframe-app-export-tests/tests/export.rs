//! The Export dialog (D9): which node an export acts on, the HTML the app layer builds for it while the dialog is open,
//! and what the report says. The export is the `export` feature's, run on the person's own view of the file (what the
//! server has not confirmed yet is in it, and so are the modes on view); nothing is written to the file.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{Action, App, ExportFormat, Property};
use meldframe_protocol::Node;
use serde_json::json;

#[test]
fn the_one_selected_frame_is_what_an_export_acts_on() {
    let mut l = Loopback::live();
    assert_eq!(l.app.state().export.target, None);
    select(&mut l, "root");
    let t = l.app.state().export.target.clone().unwrap();
    assert_eq!(
        (t.id.as_str(), t.name.as_str()),
        ("root", get(&l.app, "root").name.as_str())
    );
}

#[test]
fn nothing_else_is_exportable_a_shape_text_two_nodes_or_no_selection() {
    let mut l = Loopback::live();
    for pick in [vec!["r1"], vec!["t1"], vec!["root", "r1"], vec![]] {
        l.send(Action::Select { ids: ids(&pick) });
        assert_eq!(l.app.state().export.target, None, "{pick:?}");
    }
}

#[test]
fn opening_builds_the_document_of_the_frame_in_tailwind_first() {
    let mut l = Loopback::live();
    select(&mut l, "root");
    assert!(l.app.state().export.dialog.is_none());
    open(&mut l);
    let d = l.app.state().export.dialog.clone().unwrap();
    assert_eq!(
        (d.frame.as_str(), d.format),
        ("root", ExportFormat::Tailwind)
    );
    assert!(
        d.html.starts_with("<!doctype html>") || d.html.starts_with("<!DOCTYPE html>"),
        "{}",
        &d.html[..40]
    );
    assert!(d.html.contains("data-node=\"root\""));
    assert!(
        !d.css.is_empty(),
        "the Tailwind form has a stylesheet for the compiler"
    );
    assert!(d.html.contains(&format!("href=\"{}\"", d.css_file)));
    assert_eq!(d.error, None);
}

#[test]
fn the_format_switches_between_tailwind_and_plain_css() {
    let mut l = Loopback::live();
    select(&mut l, "root");
    open(&mut l);
    let tailwind = l.app.state().export.dialog.clone().unwrap();
    l.send(Action::SetExportFormat {
        format: ExportFormat::Css,
    });
    let css = l.app.state().export.dialog.clone().unwrap();
    assert_eq!(css.format, ExportFormat::Css);
    assert_ne!(css.html, tailwind.html);
    assert!(
        css.html.contains("<style"),
        "plain CSS carries its stylesheet inline"
    );
}

#[test]
fn nothing_is_built_while_the_dialog_is_closed() {
    let mut l = Loopback::live();
    select(&mut l, "root");
    l.send(Action::Move {
        id: "r1".into(),
        x: 5.0,
        y: 5.0,
    });
    assert_eq!(l.app.export_builds(), 0);
    open(&mut l);
    let after_open = l.app.export_builds();
    assert!(after_open >= 1);
    l.send(Action::CloseExport {});
    assert!(l.app.state().export.dialog.is_none());
    l.send(Action::Move {
        id: "r1".into(),
        x: 6.0,
        y: 6.0,
    });
    assert_eq!(l.app.export_builds(), after_open);
}

#[test]
fn opening_with_no_exportable_frame_is_refused_and_opens_nothing() {
    let mut l = Loopback::live();
    open(&mut l);
    assert_eq!(refused(&mut l), vec!["not_exportable".to_string()]);
    assert!(l.app.state().export.dialog.is_none());
    select(&mut l, "r1");
    open(&mut l);
    assert_eq!(refused(&mut l), vec!["not_exportable".to_string()]);
}

#[test]
fn the_dialog_keeps_its_frame_when_the_selection_moves_on() {
    let mut l = Loopback::live();
    select(&mut l, "root");
    open(&mut l);
    select(&mut l, "r1");
    assert_eq!(l.app.state().export.dialog.as_ref().unwrap().frame, "root");
    assert_eq!(l.app.state().export.target, None);
}

#[test]
fn the_dialog_follows_the_document_and_closes_when_its_frame_is_gone() {
    let mut l = Loopback::live();
    select(&mut l, "root");
    open(&mut l);
    let before = l.app.state().export.dialog.clone().unwrap().html;
    l.send(Action::SetProperty {
        id: "root".into(),
        property: Property::Fill("#12ab34".into()),
    });
    let after = l.app.state().export.dialog.clone().unwrap().html;
    assert_ne!(before, after);
    assert!(after.contains("#12ab34"));
    l.send(Action::Delete {
        ids: ids(&["root"]),
    });
    assert!(
        l.app.state().export.dialog.is_none(),
        "a frame that is gone cannot be exported"
    );
}

#[test]
fn the_export_has_what_the_server_has_not_confirmed_yet() {
    // No frames come back: the edit stays pending, and it is in the export all the same.
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    app.dispatch(Action::Select {
        ids: ids(&["root"]),
    });
    app.dispatch(Action::SetProperty {
        id: "root".into(),
        property: Property::Fill("#ab1234".into()),
    });
    assert_eq!(app.state().pending.len(), 1);
    app.dispatch(Action::OpenExport {});
    assert!(
        app.state()
            .export
            .dialog
            .as_ref()
            .unwrap()
            .html
            .contains("#ab1234")
    );
}

fn app_with(nodes: serde_json::Value) -> App {
    let nodes: Vec<Node> = serde_json::from_value(nodes).unwrap();
    App::new("user:me", "c0", 1, nodes, 0).unwrap()
}

#[test]
fn what_the_export_cannot_carry_is_listed_with_the_node_that_has_it() {
    let mut app = app_with(json!([
        {"id": "root", "type": "frame", "parentId": null, "name": "Landing", "w": 400, "h": 300},
        {"id": "t1", "type": "text", "parentId": "root", "name": "Hero Title", "text": "Hello",
         "shadows": [{"x": 0, "y": 2, "blur": 4, "spread": 0, "color": "#000000"}]},
    ]));
    app.dispatch(Action::Select {
        ids: ids(&["root"]),
    });
    app.dispatch(Action::OpenExport {});
    let d = app.state().export.dialog.clone().unwrap();
    assert_eq!(d.unsupported.len(), 1, "{:?}", d.unsupported);
    let f = &d.unsupported[0];
    assert_eq!(
        (f.code.as_str(), f.node.as_deref(), f.node_name.as_deref()),
        ("on_text", Some("t1"), Some("Hero Title"))
    );
    assert!(!f.message.is_empty());
}

#[test]
fn a_frame_too_large_for_the_limit_says_so_and_has_no_document() {
    let rows: Vec<serde_json::Value> = (0..2500)
        .map(|i| json!({"id": format!("n{i}"), "type": "text", "parentId": "root", "name": format!("Row {i} with a long enough name to take space"), "text": format!("Row {i} with some text that is long enough to make the output big")}))
        .collect();
    let mut nodes =
        vec![json!({"id": "root", "type": "frame", "parentId": null, "w": 400, "h": 300})];
    nodes.extend(rows);
    let mut app = app_with(json!(nodes));
    app.dispatch(Action::Select {
        ids: ids(&["root"]),
    });
    app.dispatch(Action::OpenExport {});
    let d = app.state().export.dialog.clone().unwrap();
    assert_eq!(d.error.as_deref(), Some("E_EXPORT_TOO_LARGE"));
    assert!(d.html.is_empty() && d.css.is_empty());
    assert_eq!(d.limit, 256 * 1024);
    assert!(d.counted_bytes.unwrap() > d.limit);
    assert_eq!(
        (d.html_bytes, d.css_bytes),
        (None, None),
        "no file, so no size"
    );
}

#[test]
fn the_export_is_made_in_the_modes_on_view_and_the_variables_stay_variables() {
    let nodes: Vec<Node> = serde_json::from_value(json!([
        {"id": "root", "type": "frame", "parentId": null, "w": 200, "h": 100, "fill": {"var": "bg"}},
    ]))
    .unwrap();
    let vars = serde_json::from_value(json!({
        "collections": [{"id": "t", "name": "appearance", "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}],
        "variables": [{"id": "bg", "collection": "t", "name": "color/bg", "type": "color", "values": {"light": "#ffffff", "dark": "#101112"}}],
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
    let light = app.state().export.dialog.clone().unwrap();
    assert!(
        light.html.contains("var(--"),
        "a bound fill is a CSS variable, not a literal"
    );
    assert!(!light.html.contains("data-mode=\"dark\""), "{}", light.html);
    app.dispatch(Action::SetMode {
        collection: "t".into(),
        mode: Some("dark".into()),
    });
    let dark = app.state().export.dialog.clone().unwrap();
    assert_ne!(light.html, dark.html, "the dialog follows the mode on view");
    assert!(dark.html.contains("dark"));
}
