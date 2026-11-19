//! E4.1: the work an export may do is bounded by the frame it is asked for and by the size limit, not by the file.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod support;
use export::{ExportError, ExportOptions, Format};
use meldframe_layout::{StubMeasurer, TextMeasurer};
use meldframe_protocol::{Node, Variables};
use serde_json::{Value, json};
use std::cell::Cell;
use support::*;

/// Counts the line-height probes the walk makes (one `M` per styled text node), so the work of the walk is
/// something a test can bound without a clock.
struct Counting {
    probes: Cell<usize>,
}

impl TextMeasurer for Counting {
    fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
        if node.text == "M" {
            self.probes.set(self.probes.get() + 1);
        }
        StubMeasurer.measure(node, wrap_w)
    }
}

fn counting() -> Counting {
    Counting {
        probes: Cell::new(0),
    }
}

/// `count` fixed-size text nodes with `len` characters each, in the frame `f`.
fn texts(count: usize, len: usize) -> Vec<Node> {
    let body = "x".repeat(len);
    let kids = (0..count)
        .map(|i| {
            json!({"id": format!("t{i}"), "type": "text", "text": body, "sw": "fixed", "sh": "fixed", "w": 50, "h": 20})
        })
        .collect();
    nodes(frame(json!({}), kids))
}

#[test]
fn a_frame_that_would_be_huge_is_refused_before_anything_is_built() {
    // 1,000 texts of 20,000 characters: 20 MB of HTML if it were built.
    let list = texts(1_000, 20_000);
    let m = counting();
    let options = ExportOptions::new(Format::Css, &m);
    let err = export::export_frame(&list, &Variables::default(), "f", &options).unwrap_err();
    assert_eq!(err.code(), "E_EXPORT_TOO_LARGE", "{err}");
    assert!(
        m.probes.get() < 50,
        "the work stops when the limit is passed: {} texts were styled",
        m.probes.get()
    );
}

#[test]
fn the_walk_stops_at_the_limit_when_the_size_comes_from_the_styles() {
    // Short texts, so only the styles make the output large: the size is known only while the walk builds it.
    let list = texts(4_000, 2);
    for format in [Format::Css, Format::Tailwind] {
        let m = counting();
        let options = ExportOptions::new(format, &m);
        let err = export::export_frame(&list, &Variables::default(), "f", &options).unwrap_err();
        assert!(
            matches!(err, ExportError::TooLarge { .. }),
            "{format:?}: {err}"
        );
        assert!(
            m.probes.get() < 2_500,
            "{format:?}: {} of 4,000 texts were styled before the refusal",
            m.probes.get()
        );
    }
}

#[test]
fn the_limit_counts_what_the_answer_carries_and_the_plain_css_is_in_it_twice() {
    // In the plain CSS form the stylesheet is in the html and in `css`: both come back, so both count.
    let e = css(frame(json!({}), vec![json!({"id": "r", "type": "rect"})]));
    let inline = e.html.matches(&e.css).count();
    assert_eq!(inline, 1, "the whole stylesheet is inline in the document");
    let options = ExportOptions::new(Format::Css, &StubMeasurer);
    let list = nodes(frame(json!({}), vec![json!({"id": "r", "type": "rect"})]));
    let mut options = options;
    options.max_bytes = e.html.len() + e.css.len() - 1;
    assert!(export::export_frame(&list, &Variables::default(), "f", &options).is_err());
    options.max_bytes = e.html.len() + e.css.len();
    assert!(export::export_frame(&list, &Variables::default(), "f", &options).is_ok());
}

/// Many top-level frames of different shapes, one of them the target, and the target alone.
fn big_file_and_alone(target: Value) -> (Vec<Node>, Vec<Node>) {
    let mut all = Vec::new();
    for i in 0..60 {
        all.extend(nodes(json!([
            {"id": format!("o{i}"), "type": "frame", "layout": "column", "w": 100, "h": 100, "padding": 7, "gap": 3},
            {"id": format!("o{i}a"), "type": "text", "parentId": format!("o{i}"), "pos": "a", "text": "other hug text"},
            {"id": format!("o{i}b"), "type": "rect", "parentId": format!("o{i}"), "pos": "b", "sw": "fill", "w": 10, "h": 10}
        ])));
    }
    let alone = nodes(target);
    all.extend(alone.iter().cloned());
    (all, alone)
}

fn target() -> Value {
    json!([
        {"id": "f", "type": "frame", "layout": "row", "x": 30, "y": 40, "w": 300, "h": 120, "padding": 4, "gap": 5,
            "fill": "#102030"},
        {"id": "a", "type": "frame", "parentId": "f", "pos": "a", "layout": "column", "sw": "fill", "sh": "fill",
            "padT": 9, "w": 10, "h": 10},
        {"id": "b", "type": "frame", "parentId": "f", "pos": "b", "layout": "column", "sw": "fill", "sh": "fill",
            "padding": 2, "w": 10, "h": 10},
        {"id": "t", "type": "text", "parentId": "a", "pos": "a", "text": "hello there", "sw": "fill"},
        {"id": "u", "type": "text", "parentId": "b", "pos": "a", "text": "hug"}
    ])
}

#[test]
fn the_export_of_a_frame_in_a_big_file_equals_the_export_of_the_frame_alone() {
    let (big, alone) = big_file_and_alone(target());
    for format in [Format::Css, Format::Tailwind] {
        let v = Variables::default();
        let m = counting();
        let o = ExportOptions::new(format, &m);
        let a = export::export_frame(&big, &v, "f", &o).unwrap();
        let b = export::export_frame(&alone, &v, "f", &o).unwrap();
        assert_eq!((a.html, a.css), (b.html, b.css), "{format:?}");
        assert_eq!(a.report, b.report, "{format:?}");
    }
}

#[test]
fn a_nested_frame_in_a_big_file_equals_the_same_frame_alone_at_the_origin() {
    let mut list = target();
    // The frame hangs in a column page; alone, it is a top-level frame at the origin.
    let nested = {
        let mut v = list.clone();
        let f = v[0].as_object_mut().unwrap();
        f.insert("parentId".into(), json!("page"));
        f.insert("pos".into(), json!("a"));
        let mut all = vec![
            json!({"id": "page", "type": "frame", "layout": "column", "w": 800, "h": 600, "padding": 20}),
        ];
        all.extend(v.as_array().unwrap().iter().cloned());
        Value::Array(all)
    };
    list[0]["x"] = json!(0);
    list[0]["y"] = json!(0);
    let (mut big, _) = big_file_and_alone(nested);
    big.retain(|n| !n.id.starts_with("zz"));
    let alone = nodes(list);
    let v = Variables::default();
    let o = ExportOptions::new(Format::Css, &StubMeasurer);
    let a = export::export_frame(&big, &v, "f", &o).unwrap();
    let b = export::export_frame(&alone, &v, "f", &o).unwrap();
    assert_eq!((a.html, a.css), (b.html, b.css));
}

#[test]
fn a_tree_deeper_than_the_depth_guard_is_cut_and_the_report_says_so() {
    // A file loaded from elsewhere may nest deeper than the write limit of 32; the export has its own limit.
    let mut list = vec![json!({"id": "f", "type": "frame", "w": 100, "h": 100})];
    for i in 0..500 {
        let parent = if i == 0 {
            "f".to_owned()
        } else {
            format!("d{}", i - 1)
        };
        list.push(
            json!({"id": format!("d{i}"), "type": "frame", "parentId": parent, "w": 50, "h": 50}),
        );
    }
    let list = nodes(Value::Array(list));
    let e = export::export_frame(
        &list,
        &Variables::default(),
        "f",
        &ExportOptions::new(Format::Css, &StubMeasurer),
    )
    .unwrap();
    assert!(e.html.contains("data-node=\"d10\""));
    assert!(!e.html.contains("data-node=\"d499\""));
    assert!(
        e.report.warnings.iter().any(|w| w.code == "too_deep"),
        "{:?}",
        e.report
    );
}

#[test]
fn many_fills_beside_each_other_export_the_same_basis_as_a_few() {
    // The share of a fill with unequal padding is worked out once per parent, not once per child.
    let kids: Vec<Value> = (0..300)
        .map(|i| {
            json!({"id": format!("k{i}"), "type": "frame", "layout": "column", "sw": "fill", "w": 10, "h": 10,
                "padL": (i % 3) as f64})
        })
        .collect();
    let e = css(frame(json!({"layout": "row", "w": 3000}), kids));
    let a = rule(&e, "k0").unwrap();
    let b = rule(&e, "k1").unwrap();
    assert!(a.contains("flex:1 1 ") && !a.contains("flex:1 1 0%"), "{a}");
    assert_ne!(a, b);
}
