//! E4.1: an export whose text sizes are an estimate says so. A caller that measures with the stub (the MCP tool: the
//! server has no fonts) sets `ExportOptions::estimated_text`; the report then names, in one warning, the texts whose
//! box the measurer decides (a hug width, or a hug height). A caller with the page's
//! measurer leaves it off and gets no warning. The line height of every text is the measurer's either way.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod support;
use export::{Export, ExportOptions, Format};
use meldframe_layout::StubMeasurer;
use meldframe_protocol::Variables;
use serde_json::{Value, json};
use support::*;

const CODE: &str = "text_size_estimated";
const TAIL: &str = "from an estimate of the text, not from the font the browser uses, so the size may differ from the canvas and the Export dialog:";

fn hug(id: &str) -> Value {
    json!({"id": id, "type": "text", "text": "Hello", "fontSize": 24, "sw": "hug", "sh": "hug"})
}

fn fixed(id: &str) -> Value {
    json!({"id": id, "type": "text", "text": "Fixed", "w": 90, "h": 30})
}

/// Frame `f` as the probe of issue 241: a row with two hug texts, a text of fixed size and a rect.
fn probe() -> Value {
    frame(
        json!({"layout": "row", "gap": 12, "padding": 12}),
        vec![
            hug("wide"),
            hug("narrow"),
            fixed("fixed"),
            json!({"id": "box", "type": "rect", "w": 20, "h": 20}),
        ],
    )
}

fn run_with(list: Value, estimated: bool) -> Export {
    let options = ExportOptions::new(Format::Css, &StubMeasurer).estimated_text(estimated);
    export::export_frame(&nodes(list), &Variables::default(), "f", &options).unwrap()
}

/// The messages of the warning, which there must be one of at most.
fn named(e: &Export) -> Option<String> {
    let all: Vec<_> = e
        .report
        .warnings
        .iter()
        .filter(|w| w.code == CODE)
        .collect();
    assert!(all.len() <= 1, "{:?}", e.report);
    all.first().map(|w| {
        assert_eq!(w.node, None, "it is about the export, not one node");
        w.message.clone()
    })
}

/// `n` hug texts `t0`, `t1`, ... in a column (their `pos` sorts in that order up to 10).
fn texts(n: usize) -> Value {
    let kids = (0..n).map(|i| hug(&format!("t{i}"))).collect();
    frame(json!({"layout": "column"}), kids)
}

#[test]
fn the_hug_texts_are_named_in_file_order_and_the_fixed_one_is_not() {
    for format in [Format::Css, Format::Tailwind] {
        let options = ExportOptions::new(format, &StubMeasurer).estimated_text(true);
        let e =
            export::export_frame(&nodes(probe()), &Variables::default(), "f", &options).unwrap();
        assert_eq!(
            named(&e).as_deref(),
            Some(format!("2 text boxes get their size {TAIL} wide, narrow").as_str())
        );
    }
}

#[test]
fn a_text_with_a_hug_height_only_is_named_without_a_claim_about_min_width() {
    let t = json!({"id": "t", "type": "text", "text": "Hello", "w": 90, "sh": "hug"});
    let e = run_with(frame(json!({"layout": "column"}), vec![t]), true);
    let m = named(&e).expect("named");
    assert_eq!(m, format!("1 text box gets its size {TAIL} t"));
    assert!(!m.contains("min-width"), "{m}");
}

#[test]
fn a_hug_text_in_a_nested_frame_is_named() {
    let inner = json!({"id": "in", "type": "frame", "w": 100, "h": 50, "layout": "row"});
    let mut list = frame(json!({}), vec![inner]).as_array().unwrap().clone();
    let mut t = hug("deep");
    t["parentId"] = json!("in");
    t["pos"] = json!("a0");
    list.push(t);
    let e = run_with(Value::Array(list), true);
    assert_eq!(
        named(&e).as_deref(),
        Some(format!("1 text box gets its size {TAIL} deep").as_str())
    );
}

#[test]
fn a_fill_width_with_a_hug_height_is_named_because_the_height_is_the_wrapped_one() {
    // Fill width in a column of fixed width is `align-self:stretch`, hug height is the wrapped height.
    let t = json!({"id": "t", "type": "text", "text": "Hello", "sw": "fill", "sh": "hug"});
    let e = run_with(frame(json!({"layout": "column"}), vec![t]), true);
    assert!(named(&e).is_some(), "{:?}", e.report);
}

#[test]
fn a_fill_height_is_the_layouts_and_is_not_named() {
    // Under a layout parent of fixed size the export writes flex or stretch: the layout sets the height, not the stub.
    for t in [
        json!({"id": "t", "type": "text", "text": "Hello", "w": 80, "sh": "fill"}),
        json!({"id": "t", "type": "text", "text": "Hello", "sw": "fill", "sh": "fill"}),
    ] {
        for layout in ["column", "row"] {
            let e = run_with(frame(json!({"layout": layout}), vec![t.clone()]), true);
            assert_eq!(named(&e), None, "{layout}: {:?}", e.report);
        }
    }
}

#[test]
fn a_fill_text_the_export_fixes_is_not_named() {
    // Rule G1: no layout parent, or a parent that hugs on the axis, makes a fill a fixed size.
    let t = json!({"id": "t", "type": "text", "text": "Hello", "w": 80, "h": 20, "sw": "fill", "sh": "fill"});
    let e = run_with(frame(json!({}), vec![t.clone()]), true);
    assert_eq!(named(&e), None, "{:?}", e.report);
    let hugging = frame(
        json!({"layout": "column", "sw": "hug", "sh": "hug"}),
        vec![t],
    );
    let e = run_with(hugging, true);
    assert_eq!(named(&e), None, "{:?}", e.report);
}

#[test]
fn a_frame_with_only_fixed_texts_has_no_warning() {
    let list = frame(json!({"layout": "row"}), vec![fixed("a"), fixed("b")]);
    let e = run_with(list, true);
    assert_eq!(named(&e), None, "{:?}", e.report);
}

#[test]
fn eight_texts_are_all_listed_and_nine_are_cut_and_counted() {
    let names = |n: usize| {
        (0..n)
            .map(|i| format!("t{i}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let e = run_with(texts(8), true);
    assert_eq!(
        named(&e).as_deref(),
        Some(format!("8 text boxes get their size {TAIL} {}", names(8)).as_str())
    );
    let e = run_with(texts(9), true);
    assert_eq!(
        named(&e).as_deref(),
        Some(format!("9 text boxes get their size {TAIL} {} and 1 more", names(8)).as_str())
    );
}

#[test]
fn the_page_measurer_gets_no_warning_and_the_same_bytes() {
    let (on, off) = (run_with(probe(), true), run_with(probe(), false));
    assert_eq!(named(&off), None, "{:?}", off.report);
    assert_eq!((on.html, on.css), (off.html, off.css));
    // `ExportOptions::new` declares no estimate.
    let options = ExportOptions::new(Format::Css, &StubMeasurer);
    let e = export::export_frame(&nodes(probe()), &Variables::default(), "f", &options).unwrap();
    assert_eq!(named(&e), None, "{:?}", e.report);
}
