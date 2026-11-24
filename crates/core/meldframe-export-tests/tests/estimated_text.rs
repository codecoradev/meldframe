//! E4.1: an export whose text sizes are an estimate says so. A caller that measures with the stub (the MCP tool: the
//! server has no fonts) sets `ExportOptions::estimated_text`; the report then names the text whose box the
//! measurer decides (`sw` or `sh` hug or fill). A caller with the page's measurer leaves it off and gets no warning.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod support;
use export::{Export, ExportOptions, Format};
use meldframe_layout::StubMeasurer;
use meldframe_protocol::Variables;
use serde_json::{Value, json};
use support::*;

const CODE: &str = "text_size_estimated";

/// Frame `f` as the probe of issue 241: a row with two hug texts, a text of fixed size and a rect.
fn probe() -> Value {
    let hug = |id: &str, text: &str, weight: u32| json!({"id": id, "type": "text", "text": text, "fontSize": 24, "fontWeight": weight, "sw": "hug", "sh": "hug"});
    frame(
        json!({"layout": "row", "gap": 12, "padding": 12}),
        vec![
            hug("wide", "WWWWWWWWWW MMMMMMMM", 700),
            hug("narrow", "illiiiliiil lll", 400),
            json!({"id": "fixed", "type": "text", "text": "Fixed", "w": 90, "h": 30}),
            json!({"id": "box", "type": "rect", "w": 20, "h": 20}),
        ],
    )
}

fn export_with(list: Value, format: Format, estimated: bool) -> Export {
    let options = ExportOptions::new(format, &StubMeasurer).estimated_text(estimated);
    export::export_frame(&nodes(list), &Variables::default(), "f", &options).unwrap()
}

fn estimated(e: &Export) -> Vec<&export::Finding> {
    e.report
        .warnings
        .iter()
        .filter(|w| w.code == CODE)
        .collect()
}

#[test]
fn a_hug_text_under_an_estimating_measurer_is_one_warning_naming_the_texts() {
    for format in [Format::Css, Format::Tailwind] {
        let e = export_with(probe(), format, true);
        let found = estimated(&e);
        assert_eq!(found.len(), 1, "{:?}", e.report);
        let w = found[0];
        assert_eq!(w.node, None, "it is about the export, not one node");
        for want in ["2 text", "wide", "narrow"] {
            assert!(w.message.contains(want), "{want}: {}", w.message);
        }
        assert!(
            !w.message.contains("fixed"),
            "a text of fixed size is not listed: {}",
            w.message
        );
    }
}

#[test]
fn a_fill_text_counts_too() {
    let t = json!({"id": "t", "type": "text", "text": "Hello", "sw": "fill", "sh": "hug"});
    let e = export_with(
        frame(json!({"layout": "column"}), vec![t]),
        Format::Css,
        true,
    );
    assert_eq!(estimated(&e).len(), 1, "{:?}", e.report);
}

#[test]
fn the_page_measurer_gets_no_warning() {
    let e = export_with(probe(), Format::Css, false);
    assert!(estimated(&e).is_empty(), "{:?}", e.report);
    // The default of `ExportOptions::new` is the measurer's own claim: no estimate is declared.
    let options = ExportOptions::new(Format::Css, &StubMeasurer);
    let e = export::export_frame(&nodes(probe()), &Variables::default(), "f", &options).unwrap();
    assert!(estimated(&e).is_empty(), "{:?}", e.report);
}

#[test]
fn no_hug_text_no_warning() {
    let list = frame(
        json!({"layout": "row"}),
        vec![
            json!({"id": "fixed", "type": "text", "text": "Fixed", "w": 90, "h": 30}),
            json!({"id": "box", "type": "rect", "w": 20, "h": 20}),
        ],
    );
    let e = export_with(list, Format::Css, true);
    assert!(estimated(&e).is_empty(), "{:?}", e.report);
}

#[test]
fn a_long_list_of_texts_is_cut_and_counted() {
    let kids = (0..12)
        .map(|i| json!({"id": format!("t{i}"), "type": "text", "text": "x", "sw": "hug", "sh": "hug"}))
        .collect();
    let e = export_with(frame(json!({"layout": "column"}), kids), Format::Css, true);
    let found = estimated(&e);
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("12 text"), "{}", found[0].message);
    assert!(found[0].message.contains("4 more"), "{}", found[0].message);
    assert!(!found[0].message.contains("t9"), "{}", found[0].message);
}

#[test]
fn the_pinned_sizes_do_not_change() {
    // The flag only adds the warning: the CSS is the same bytes.
    let (a, b) = (
        export_with(probe(), Format::Css, true),
        export_with(probe(), Format::Css, false),
    );
    assert_eq!((a.html, a.css), (b.html, b.css));
}
