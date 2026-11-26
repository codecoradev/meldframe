//! E3.2: the layout findings of the fidelity contract (rules G1 to G3). They only report: the layout itself is
//! unchanged, and a node a finding names is still laid out as before. Every number is worked out by hand.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::node;
use meldframe_layout::{StubMeasurer, layout_findings};
use serde_json::json;

fn codes(nodes: &[meldframe_protocol::Node]) -> Vec<(String, &'static str)> {
    layout_findings(nodes, &StubMeasurer)
        .into_iter()
        .map(|f| (f.id, f.code))
        .collect()
}

fn row(extra: serde_json::Value) -> meldframe_protocol::Node {
    let mut v = json!({"layout": "row", "w": 200, "h": 100, "padding": 0, "gap": 0});
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    node("f", "frame", None, v)
}

#[test]
fn g1_fill_under_a_hug_parent_is_fixed_and_reported_per_axis() {
    let nodes = [
        row(json!({"sw": "hug"})),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"sw": "fill", "w": 30, "h": 20}),
        ),
    ];
    // The parent hugs its width, so the child's fill width is its own 30: reported once, for the width.
    assert_eq!(codes(&nodes), [("a".to_string(), "fill_under_hug")]);
    let f = &layout_findings(&nodes, &StubMeasurer)[0];
    assert!(f.message.contains("width"), "{}", f.message);
}

#[test]
fn g1_fill_in_a_frame_without_layout_is_fixed_too() {
    let nodes = [
        node("f", "frame", None, json!({"w": 200, "h": 100})),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"sh": "fill", "w": 30, "h": 20}),
        ),
    ];
    let f = layout_findings(&nodes, &StubMeasurer);
    assert_eq!(f.len(), 1);
    assert!(f[0].message.contains("height"), "{}", f[0].message);
}

#[test]
fn g1_does_not_flag_fill_in_a_fixed_layout_frame_or_at_the_top_level() {
    let nodes = [
        row(json!({})),
        node("a", "rect", Some("f"), json!({"sw": "fill", "h": 20})),
        node(
            "top",
            "rect",
            None,
            json!({"sw": "fill", "x": 500, "w": 10, "h": 10}),
        ),
    ];
    assert_eq!(codes(&nodes), []);
}

#[test]
fn g3_children_that_do_not_fit_the_content_box_overflow() {
    let nodes = [
        row(json!({"gap": 10, "padding": 5})),
        node("a", "rect", Some("f"), json!({"w": 100, "h": 20})),
        node("b", "rect", Some("f"), json!({"w": 100, "h": 20})),
    ];
    // Content width 200 - 10 = 190; children 100 + 10 + 100 = 210.
    assert_eq!(codes(&nodes), [("f".to_string(), "overflow")]);
}

#[test]
fn g3_children_that_fit_exactly_do_not_overflow_and_margins_count() {
    let fits = [
        row(json!({})),
        node("a", "rect", Some("f"), json!({"w": 100, "h": 20})),
        node("b", "rect", Some("f"), json!({"w": 100, "h": 20})),
    ];
    assert_eq!(codes(&fits), []);
    let margins = [
        row(json!({})),
        node(
            "a",
            "rect",
            Some("f"),
            json!({"w": 100, "h": 20, "marR": 5}),
        ),
        node("b", "rect", Some("f"), json!({"w": 100, "h": 20})),
    ];
    assert_eq!(codes(&margins), [("f".to_string(), "overflow")]);
}

#[test]
fn g3_a_hug_frame_cannot_overflow_and_an_absolute_child_takes_no_room() {
    let hug = [
        row(json!({"sw": "hug"})),
        node("a", "rect", Some("f"), json!({"w": 300, "h": 20})),
    ];
    assert_eq!(codes(&hug), []);
    let abs = [
        row(json!({})),
        node("a", "rect", Some("f"), json!({"w": 150, "h": 20})),
        node(
            "p",
            "rect",
            Some("f"),
            json!({"absolute": true, "w": 900, "h": 20}),
        ),
    ];
    assert_eq!(codes(&abs), []);
}

#[test]
fn g3_a_column_overflows_on_its_height() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({"layout": "column", "w": 100, "h": 50, "padding": 0, "gap": 0}),
        ),
        node("a", "rect", Some("f"), json!({"w": 10, "h": 40})),
        node("b", "rect", Some("f"), json!({"w": 10, "h": 40})),
    ];
    assert_eq!(codes(&nodes), [("f".to_string(), "overflow")]);
}

#[test]
fn g3_a_fill_that_resolves_to_less_than_one_pixel() {
    let nodes = [
        row(json!({})),
        node("a", "rect", Some("f"), json!({"w": 200, "h": 20})),
        node("b", "rect", Some("f"), json!({"sw": "fill", "h": 20})),
    ];
    // a takes the whole 200: b's share is 0.
    let got = codes(&nodes);
    assert!(got.contains(&("b".to_string(), "zero_fill")), "{got:?}");
}

#[test]
fn g3_a_box_smaller_than_its_own_padding() {
    let nodes = [node(
        "f",
        "frame",
        None,
        json!({"layout": "row", "w": 20, "h": 100, "padding": 16}),
    )];
    // Horizontal padding 32 in a width of 20.
    assert_eq!(codes(&nodes), [("f".to_string(), "smaller_than_padding")]);
}

#[test]
fn a_frame_with_hug_text_among_its_children_is_not_measured() {
    // The stub measurer is not the browser's: a frame whose fit depends on text is left alone.
    let nodes = [
        row(json!({"w": 10})),
        node(
            "t",
            "text",
            Some("f"),
            json!({"sw": "hug", "text": "a long line of text that cannot fit", "fontSize": 16}),
        ),
    ];
    assert_eq!(codes(&nodes), []);
}

#[test]
fn a_tidy_file_has_no_findings_and_the_layout_is_unchanged_by_asking() {
    let nodes = [
        row(json!({})),
        node("a", "rect", Some("f"), json!({"w": 50, "h": 20})),
        node("b", "rect", Some("f"), json!({"sw": "fill", "h": 20})),
    ];
    let before = meldframe_layout::compute_rects(&nodes, &StubMeasurer);
    assert_eq!(codes(&nodes), []);
    assert_eq!(
        meldframe_layout::compute_rects(&nodes, &StubMeasurer),
        before
    );
}

#[test]
fn g1_on_the_cross_axis_of_a_hug_parent_says_the_canvas_gives_it_the_whole() {
    // A row that hugs its height: the core stretches a child that fills the height to the tallest sibling (40),
    // so the finding must not say that the child keeps its own 10.
    let nodes = [
        row(json!({"sh": "hug"})),
        node("a", "rect", Some("f"), json!({"w": 30, "h": 40})),
        node(
            "b",
            "rect",
            Some("f"),
            json!({"sh": "fill", "w": 30, "h": 10}),
        ),
    ];
    let f = layout_findings(&nodes, &StubMeasurer);
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!((f[0].id.as_str(), f[0].code), ("b", "fill_under_hug"));
    assert!(
        f[0].message.contains("the whole of the height"),
        "{}",
        f[0].message
    );
    assert!(
        !f[0].message.contains("it keeps its own"),
        "{}",
        f[0].message
    );
}

#[test]
fn a_frame_with_hug_text_deeper_down_is_left_out_of_the_size_rules() {
    // A fixed row of 100 holds a button that hugs a text that hugs: the stub measurer's width of the text is not
    // the browser's, so the fit of the row cannot be told and no overflow is reported for it.
    let nodes = [
        row(json!({"w": 100})),
        node(
            "btn",
            "frame",
            Some("f"),
            json!({"layout": "row", "sw": "hug", "sh": "hug", "padding": 8}),
        ),
        node(
            "label",
            "text",
            Some("btn"),
            json!({"sw": "hug", "sh": "hug", "text": "A rather long button label here"}),
        ),
    ];
    assert_eq!(codes(&nodes), []);
}

#[test]
fn g1_in_a_hug_column_the_width_is_the_cross_axis_and_the_height_the_main() {
    let nodes = [
        node(
            "f",
            "frame",
            None,
            json!({"layout": "column", "sw": "hug", "sh": "hug", "padding": 0, "gap": 0}),
        ),
        node("a", "rect", Some("f"), json!({"w": 50, "h": 20})),
        node(
            "b",
            "rect",
            Some("f"),
            json!({"sw": "fill", "sh": "fill", "w": 10, "h": 10}),
        ),
    ];
    let f = layout_findings(&nodes, &StubMeasurer);
    let says = |axis: &str| {
        f.iter()
            .find(|x| x.id == "b" && x.message.contains(&format!("fill on the {axis}")))
            .map(|x| x.message.clone())
            .unwrap()
    };
    assert!(says("width").contains("the whole of the width"), "{f:?}");
    assert!(says("height").contains("a share of the height"), "{f:?}");
    // What the canvas does: the column hugs the widest child (50), and b fills that whole width.
    let rects = meldframe_layout::compute_rects(&nodes, &StubMeasurer);
    let b = rects
        .iter()
        .find(|(id, _)| id == "b")
        .map(|(_, r)| r.w)
        .unwrap();
    assert_eq!(b, 50.0);
}

#[test]
fn hug_text_inside_a_hug_frame_without_layout_does_not_skip_the_size_rules() {
    // A frame without layout keeps its own size even when it says hug, so the row around it can be checked: 150 in a
    // content box of 100 overflows.
    let nodes = [
        row(json!({"w": 100})),
        node(
            "box",
            "frame",
            Some("f"),
            json!({"sw": "hug", "sh": "hug", "w": 150, "h": 20}),
        ),
        node(
            "label",
            "text",
            Some("box"),
            json!({"sw": "hug", "sh": "hug", "text": "Label"}),
        ),
    ];
    assert_eq!(codes(&nodes), [("f".to_string(), "overflow")]);
}
