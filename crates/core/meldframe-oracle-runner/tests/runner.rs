//! The runner of the fidelity oracle: the table measurer, the rects and the two forms of the export.

use meldframe_layout::TextMeasurer;
use meldframe_oracle_runner::{Entry, Input, Query, TableMeasurer, run};
use meldframe_protocol::Node;
use serde_json::json;

fn input(table: serde_json::Value) -> Input {
    serde_json::from_value(json!({
        "fixtures": [{
            "name": "one",
            "nodes": [
                {"id": "f", "type": "frame", "x": 5, "y": 7, "w": 200, "h": 100, "layout": "column", "padding": 10, "gap": 4},
                {"id": "t", "type": "text", "parentId": "f", "text": "Hello", "fontSize": 20, "sw": "hug", "sh": "hug", "fontWeight": 700},
                {"id": "r", "type": "rect", "parentId": "f", "w": 30, "h": 30}
            ]
        }],
        "table": table
    }))
    .unwrap()
}

#[test]
fn a_query_the_table_lacks_is_a_miss_and_is_answered_by_the_stub() {
    let node: Node = serde_json::from_value(
        json!({"id": "t", "type": "text", "text": "Hi", "fontSize": 10, "fontWeight": 700}),
    )
    .unwrap();
    let m = TableMeasurer::new(&[]);
    // the stub: width ceil(2 * 5), height ceil(12)
    assert_eq!(m.measure(&node, None), (10.0, 12.0));
    m.measure(&node, None); // asked twice, missed once
    assert_eq!(
        m.misses(),
        vec![Query {
            text: "Hi".into(),
            font_size: 10.0,
            font_weight: 700,
            wrap_w: None
        }]
    );
}

#[test]
fn a_recorded_answer_is_used_and_keyed_by_weight_and_wrap_width() {
    let table = json!([
        {"text": "Hello", "fontSize": 20, "fontWeight": 700, "wrapW": null, "w": 57, "h": 25},
        {"text": "Hello", "fontSize": 20, "fontWeight": 400, "wrapW": null, "w": 51, "h": 25},
        {"text": "Hello", "fontSize": 20, "fontWeight": 700, "wrapW": 30.5, "w": 30, "h": 50}
    ]);
    let out = run(&input(table));
    let one = &out.fixtures[0];
    assert_eq!(
        one.rects["t"].w, 57.0,
        "the weight 700 entry, not the 400 one"
    );
    assert_eq!(one.rects["t"].h, 25.0);
    assert!(
        one.misses
            .iter()
            .all(|q| q.wrap_w.is_some() || q.text == "M"),
        "only the line height query ('M') is missing: {:?}",
        one.misses
    );
}

#[test]
fn a_wrap_width_one_bit_away_finds_the_same_answer() {
    let w = 119.33333333333333_f64;
    let table =
        json!([{"text": "Hi", "fontSize": 10, "fontWeight": 400, "wrapW": w, "w": 7, "h": 13}]);
    let m = TableMeasurer::new(&serde_json::from_value::<Vec<Entry>>(table).unwrap());
    let node: Node =
        serde_json::from_value(json!({"id": "t", "type": "text", "text": "Hi", "fontSize": 10}))
            .unwrap();
    assert_eq!(
        m.measure(&node, Some(f64::from_bits(w.to_bits() + 1))),
        (7.0, 13.0)
    );
    assert!(m.misses().is_empty());
}

#[test]
fn every_top_level_frame_is_exported_in_both_forms() {
    let out = run(&input(json!([])));
    let one = &out.fixtures[0];
    assert_eq!(one.rects["f"].x, 5.0);
    assert_eq!(one.rects["r"].y, 7.0 + 10.0 + one.rects["t"].h + 4.0);
    assert_eq!(one.frames.len(), 1);
    let f = &one.frames[0];
    assert_eq!(f.id, "f");
    let css = f.css.as_ref().expect("css form");
    let tw = f.tailwind.as_ref().expect("tailwind form");
    assert!(css.html.contains("data-node=\"t\""));
    assert!(css.css.contains("font-weight:700"));
    assert!(tw.html.contains("class=\""));
    assert!(tw.css.contains("@import \"tailwindcss/utilities.css\""));
    assert!(one.errors.is_empty());
}

#[test]
fn a_root_that_is_not_a_frame_is_reported_and_not_exported() {
    let i: Input = serde_json::from_value(json!({
        "fixtures": [{"name": "x", "nodes": [{"id": "r", "type": "rect"}]}],
        "table": []
    }))
    .unwrap();
    let out = run(&i);
    assert!(out.fixtures[0].frames.is_empty());
    assert_eq!(out.fixtures[0].errors.len(), 1);
    assert!(out.fixtures[0].errors[0].contains("E_NOT_A_FRAME"));
}

/// A fixture with one colour variable in two modes and a frame whose fill is bound to it.
fn bound(modes: serde_json::Value) -> Input {
    serde_json::from_value(json!({
        "fixtures": [{
            "name": "bound",
            "variables": {
                "collections": [{"id": "c", "name": "Brand", "modes": [{"id": "l", "name": "Light"}, {"id": "d", "name": "Dark"}]}],
                "variables": [{"id": "v", "collection": "c", "name": "color/primary", "type": "color", "values": {"l": "#ff0000", "d": "#0000ff"}}]
            },
            "modes": modes,
            "nodes": [{"id": "f", "type": "frame", "w": 50, "h": 50, "fill": {"var": "v"}}]
        }],
        "table": []
    }))
    .unwrap()
}

#[test]
fn the_variables_and_the_picked_modes_of_a_fixture_reach_the_export() {
    let light = run(&bound(json!({})));
    let f = &light.fixtures[0].frames[0];
    let css = &f.css.as_ref().expect("css form").css;
    assert!(css.contains("--brand-color-primary:#ff0000"), "{css}");
    assert!(css.contains("var(--brand-color-primary)"), "{css}");
    let tag = |html: &str| {
        let from = &html[html.find("<html").unwrap()..];
        from[..from.find('>').unwrap()].to_owned()
    };
    assert!(!tag(&f.css.as_ref().unwrap().html).contains("data-mode-brand"));
    let dark = run(&bound(json!({"c": "d"})));
    let html = tag(&dark.fixtures[0].frames[0].css.as_ref().unwrap().html);
    assert!(html.contains("data-mode-brand=\"dark\""), "{html}");
}
