//! E4.1: the structure of an export: elements, absolute roots, the document, the font.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod support;
use export::{ExportError, ExportOptions, Format};
use meldframe_layout::StubMeasurer;
use meldframe_protocol::Variables;
use serde_json::json;
use support::*;

#[test]
fn a_root_frame_is_an_absolute_div_at_its_own_position() {
    let e = css(frame(
        json!({}),
        vec![json!({"id": "r", "type": "rect", "x": 5, "y": 6, "w": 50, "h": 40})],
    ));
    assert!(e.html.contains("<div data-node=\"f\""), "{}", e.html);
    assert!(e.html.contains("<div data-node=\"r\""), "{}", e.html);
    let f = rule(&e, "f").expect("a rule for the frame");
    assert!(f.contains("position:absolute;left:10px;top:20px"), "{f}");
    assert!(f.contains("width:400px;height:200px"), "{f}");
    let r = rule(&e, "r").unwrap();
    assert!(
        r.contains("position:absolute;left:5px;top:6px"),
        "a child of a frame without layout sits at its own x and y: {r}"
    );
}

#[test]
fn text_is_a_paragraph_with_its_text_escaped() {
    let e = css(frame(
        json!({}),
        vec![json!({"id": "t", "type": "text", "text": "a < b & \"c\""})],
    ));
    assert!(e.html.contains("<p data-node=\"t\""), "{}", e.html);
    assert!(
        e.html.contains(">a &lt; b &amp; &quot;c&quot;</p>"),
        "{}",
        e.html
    );
}

#[test]
fn the_page_carries_the_font_face_and_the_reset() {
    let e = css(frame(json!({}), vec![]));
    assert!(e.css.contains("@font-face{font-family:'Inter Variable';src:url('inter.woff2') format('woff2');font-weight:100 900;font-display:block}"), "{}", e.css);
    assert!(e.css.contains("p{margin:0}"), "{}", e.css);
    assert!(e.html.starts_with("<!doctype html>"), "{}", e.html);
    assert!(
        e.html.contains("<style>"),
        "the plain CSS is inline: {}",
        e.html
    );
}

#[test]
fn an_exported_frame_that_is_nested_sits_at_the_origin() {
    let mut list = frame(
        json!({"layout": "row"}),
        vec![json!({"id": "g", "type": "frame", "x": 99, "y": 99})],
    );
    list[1]["id"] = json!("g");
    let n = nodes(list);
    let options = ExportOptions::new(Format::Css, &StubMeasurer);
    let e = export::export_frame(&n, &Variables::default(), "g", &options).unwrap();
    assert!(
        rule(&e, "g").unwrap().contains("left:0px;top:0px"),
        "{}",
        e.css
    );
    assert!(
        !e.html.contains("data-node=\"f\""),
        "only the frame and what is inside it: {}",
        e.html
    );
}

#[test]
fn a_missing_or_not_a_frame_target_is_refused() {
    let n = nodes(frame(json!({}), vec![json!({"id": "r", "type": "rect"})]));
    let options = ExportOptions::new(Format::Css, &StubMeasurer);
    let v = Variables::default();
    assert_eq!(
        export::export_frame(&n, &v, "nope", &options).unwrap_err(),
        ExportError::NotFound
    );
    assert_eq!(
        export::export_frame(&n, &v, "r", &options).unwrap_err(),
        ExportError::NotAFrame
    );
}

#[test]
fn the_same_file_gives_the_same_bytes() {
    let list = frame(
        json!({"layout": "column"}),
        vec![
            json!({"id": "a", "type": "rect"}),
            json!({"id": "t", "type": "text", "text": "x"}),
        ],
    );
    let (a, b) = (css(list.clone()), css(list));
    assert_eq!(
        (a.html.as_str(), a.css.as_str()),
        (b.html.as_str(), b.css.as_str())
    );
}
