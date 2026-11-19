//! E4.1: typography, one test per row of section 4.3 of the fidelity contract that the model has (font size and
//! weight, text align, text a browser breaks differently).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod support;
use serde_json::json;
use support::*;

#[test]
fn typography_is_size_weight_and_alignment() {
    let t = json!({"id": "t", "type": "text", "text": "x", "fontSize": 13.5, "fontWeight": 700, "textAlign": "right", "w": 300, "h": 30});
    let e = css(frame(json!({}), vec![t]));
    let s = rule(&e, "t").unwrap();
    assert!(
        s.contains("font-size:13.5px;font-weight:700;line-height:17px"),
        "{s}"
    );
    assert!(s.contains("text-align:right"), "{s}");
    let t = json!({"id": "t", "type": "text", "text": "x"});
    let e = css(frame(json!({}), vec![t]));
    assert!(
        !rule(&e, "t").unwrap().contains("text-align"),
        "left is the default"
    );
    assert!(rule(&e, "t").unwrap().contains("font-weight:400"));
}

#[test]
fn text_a_browser_breaks_differently_is_a_warning() {
    for text in ["a\tb", "see https://example.com", "日本語"] {
        let e = css(frame(
            json!({}),
            vec![json!({"id": "t", "type": "text", "text": text})],
        ));
        assert!(
            e.report
                .warnings
                .iter()
                .any(|w| w.code == "text_outside_contract"),
            "{text}"
        );
    }
}
