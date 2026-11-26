//! A size or a count that is not known is not zero (rule U9, finding T19 of the thin-UI audit): an export that has no
//! file has no file size, an import of text that was not read has no counts. A count that is known to be zero stays
//! zero.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{Action, ExportFormat};
use meldframe_app_model::{ImportStatus, VariablesImportResult};
use serde_json::json;

fn import(l: &mut Loopback, text: &str) -> VariablesImportResult {
    l.send(Action::ImportVariables { text: text.into() });
    l.app.state().variables_io.import.clone().unwrap()
}

#[test]
fn an_export_with_no_file_has_no_size_and_the_limit_gives_the_size_it_refused() {
    let rows: Vec<serde_json::Value> = (0..2500)
        .map(|i| json!({"id": format!("n{i}"), "type": "text", "parentId": "root", "name": format!("Row {i} with a long enough name to take space"), "text": format!("Row {i} with some text that is long enough to make the output big")}))
        .collect();
    let mut nodes =
        vec![json!({"id": "root", "type": "frame", "parentId": null, "w": 400, "h": 300})];
    nodes.extend(rows);
    let nodes = serde_json::from_value(json!(nodes)).unwrap();
    let mut app = meldframe_app::App::new("user:me", "c0", 1, nodes, 0).unwrap();
    app.dispatch(Action::Select {
        ids: ids(&["root"]),
    });
    app.dispatch(Action::OpenExport {});
    let d = app.state().export.dialog.clone().unwrap();
    assert_eq!(d.error.as_deref(), Some("E_EXPORT_TOO_LARGE"));
    assert_eq!((d.html_bytes, d.css_bytes), (None, None));
    assert!(d.counted_bytes.is_some_and(|n| n > d.limit));
    // The same frame in plain CSS is refused the same way.
    app.dispatch(Action::SetExportFormat {
        format: ExportFormat::Css,
    });
    let d = app.state().export.dialog.clone().unwrap();
    assert_eq!((d.html_bytes, d.css_bytes), (None, None));
}

#[test]
fn a_built_export_knows_every_size_and_plain_css_has_a_stylesheet_of_zero_bytes() {
    let mut l = Loopback::live();
    select(&mut l, "root");
    open(&mut l);
    let d = l.app.state().export.dialog.clone().unwrap();
    assert!(d.html_bytes.is_some() && d.counted_bytes.is_some());
    assert!(d.css_bytes.is_some_and(|n| n > 0));
    l.send(Action::SetExportFormat {
        format: ExportFormat::Css,
    });
    let d = l.app.state().export.dialog.clone().unwrap();
    assert_eq!(d.css_bytes, Some(0), "no second file: zero, not unknown");
}

#[test]
fn text_that_was_not_read_has_no_report_and_text_that_was_read_has_one() {
    let mut l = Loopback::live();
    // An empty text, and one over the limit, are refused before they are read: no counts exist.
    let r = import(&mut l, "   ");
    assert_eq!(
        (r.status, r.code.as_deref()),
        (ImportStatus::Refused, Some("empty_text"))
    );
    assert_eq!(r.report, None);
    let big = format!(":root {{ --a: #fff; }}{}", " ".repeat(256 * 1024));
    let r = import(&mut l, &big);
    assert_eq!(r.code.as_deref(), Some("E_IMPORT_TOO_LARGE"));
    assert_eq!(r.report, None);
    // Text with no variable was read: its counts are zero, and that is known.
    let r = import(&mut l, "body { color: red; }");
    assert_eq!(r.status, ImportStatus::Nothing);
    let rep = r.report.expect("the text was read");
    assert_eq!((rep.variables, rep.collections), (0, 0));
    // A text that is read and then refused keeps its report: it says why.
    let r = import(&mut l, ":root { --a: #fff; }");
    assert_eq!(r.status, ImportStatus::Imported);
    let again = import(&mut l, ":root { --a: #fff; }");
    assert_eq!(again.status, ImportStatus::Refused);
    assert!(again.report.is_some_and(|rep| rep.variables == 1));
}

#[test]
fn the_export_of_variables_knows_its_size_unless_it_refused() {
    let mut l = Loopback::live();
    import(&mut l, ":root { --a: #fff; }");
    l.send(Action::OpenVariablesExport {});
    let d = l.app.state().variables_io.export.clone().unwrap();
    assert_eq!((d.error, d.bytes), (None, Some(d.css.len())));
}
