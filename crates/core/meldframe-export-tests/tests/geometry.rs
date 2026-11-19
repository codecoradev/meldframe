//! E4.1: geometry, one test per row of the supported table of the fidelity contract (section 4.1) and per rule
//! (G1, G2, G4). The pixel oracle that renders these is E4.2; here the generated text is the probe.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod support;
use serde_json::json;
use support::*;

fn has(rule: Option<&str>, parts: &[&str]) {
    let r = rule.expect("a rule");
    for p in parts {
        assert!(r.contains(p), "`{p}` is not in `{r}`");
    }
}

#[test]
fn a_fixed_size_in_a_row_does_not_shrink_on_the_main_axis_only() {
    let e = css(frame(
        json!({"layout": "row"}),
        vec![json!({"id": "r", "type": "rect", "w": 50, "h": 40})],
    ));
    has(rule(&e, "r"), &["width:50px;height:40px;flex-shrink:0"]);
    let e = css(frame(
        json!({"layout": "column"}),
        vec![json!({"id": "r", "type": "rect", "w": 50, "h": 40})],
    ));
    has(rule(&e, "r"), &["width:50px;height:40px;flex-shrink:0"]);
    // In a column the main axis is the height, in a row the width: one flex-shrink either way, and none without layout.
    let e = css(frame(json!({}), vec![json!({"id": "r", "type": "rect"})]));
    assert!(!rule(&e, "r").unwrap().contains("flex-shrink"));
}

#[test]
fn a_hug_text_is_pinned_to_what_the_core_measured() {
    // The stub measures a character as half the font size and a line as 1.2 times it, rounded up (rule G4).
    let t = json!({"id": "t", "type": "text", "text": "Hello", "fontSize": 20, "sw": "hug", "sh": "hug"});
    let e = css(frame(json!({"layout": "row"}), vec![t]));
    has(
        rule(&e, "t"),
        &[
            "width:auto;min-width:50px",
            "height:auto",
            "line-height:24px",
            "white-space:pre;",
        ],
    );
    assert!(e.html.contains(">Hello</p>"));
}

#[test]
fn the_line_height_of_text_with_newlines_is_one_line() {
    let t = json!({"id": "t", "type": "text", "text": "a\nb\nc", "fontSize": 20, "sw": "hug", "sh": "hug"});
    let e = css(frame(json!({"layout": "row"}), vec![t]));
    has(rule(&e, "t"), &["line-height:24px"]);
}

#[test]
fn text_with_a_width_wraps_and_keeps_its_spaces() {
    let t = json!({"id": "t", "type": "text", "text": "a  b", "w": 90, "h": 40});
    let e = css(frame(json!({}), vec![t]));
    has(
        rule(&e, "t"),
        &[
            "width:90px;height:40px",
            "white-space:pre-wrap;word-break:keep-all",
        ],
    );
}

#[test]
fn a_hug_layout_frame_is_fit_content_wide_and_auto_high() {
    let g = json!({"id": "g", "type": "frame", "layout": "column", "sw": "hug", "sh": "hug"});
    let e = css(frame(json!({}), vec![g]));
    has(rule(&e, "g"), &["width:fit-content", "height:auto"]);
}

#[test]
fn a_hug_that_is_not_a_layout_frame_is_fixed_as_in_the_core() {
    let r = json!({"id": "r", "type": "rect", "w": 30, "h": 20, "sw": "hug", "sh": "hug"});
    let e = css(frame(json!({}), vec![r]));
    has(rule(&e, "r"), &["width:30px;height:20px"]);
}

#[test]
fn fill_shares_the_main_axis_and_stretches_the_cross_axis() {
    let a = json!({"id": "a", "type": "rect", "sw": "fill", "sh": "fill"});
    let e = css(frame(json!({"layout": "row"}), vec![a]));
    let r = rule(&e, "a").unwrap();
    assert!(
        r.contains("flex:1 1 0%;min-width:0") && r.contains("align-self:stretch"),
        "{r}"
    );
    assert!(
        !r.contains("width:") || r.contains("min-width"),
        "no fixed width: {r}"
    );
    let c = css(frame(
        json!({"layout": "column"}),
        vec![json!({"id": "a", "type": "rect", "sw": "fill", "sh": "fill"})],
    ));
    let r = rule(&c, "a").unwrap();
    assert!(
        r.contains("flex:1 1 0%;min-height:0") && r.contains("align-self:stretch"),
        "{r}"
    );
}

#[test]
fn fill_siblings_with_unequal_padding_take_the_size_the_core_gave() {
    let pad = |id: &str, p: u32| json!({"id": id, "type": "frame", "layout": "row", "sw": "fill", "sh": "fixed", "h": 20, "padding": p});
    let e = css(frame(
        json!({"layout": "row", "padding": 0, "gap": 0}),
        vec![pad("a", 0), pad("b", 20)],
    ));
    // Equal border boxes of 200 px each, which `flex: 1 1 0%` would not give once `b` has padding.
    has(rule(&e, "a"), &["flex:1 1 200px"]);
    has(rule(&e, "b"), &["flex:1 1 200px"]);
}

#[test]
fn rule_g1_a_fill_under_a_hug_parent_or_without_layout_is_a_fixed_size() {
    let under_hug = json!({"id": "g", "type": "frame", "layout": "row", "sw": "hug", "sh": "hug", "padding": 0, "gap": 0});
    let kid = json!({"id": "k", "type": "rect", "parentId": "g", "pos": "a", "sw": "fill", "sh": "fill", "w": 60, "h": 30});
    let mut list = frame(json!({}), vec![under_hug]);
    list.as_array_mut().unwrap().push(kid);
    let e = css(list);
    has(rule(&e, "k"), &["width:60px;height:30px"]);
    assert!(!rule(&e, "k").unwrap().contains("flex:1"));
    let e = css(frame(
        json!({}),
        vec![json!({"id": "r", "type": "rect", "sw": "fill", "sh": "fill", "w": 70, "h": 10})],
    ));
    has(rule(&e, "r"), &["width:70px;height:10px"]);
    assert!(
        e.report
            .warnings
            .iter()
            .any(|w| w.code == "fill_under_hug" && w.node.as_deref() == Some("r")),
        "{:?}",
        e.report
    );
}

#[test]
fn layout_gap_padding_and_alignment() {
    let f =
        json!({"layout": "row", "gap": 12, "padding": 16, "align": "center", "justify": "between"});
    let e = css(frame(f, vec![]));
    has(
        rule(&e, "f"),
        &[
            "display:flex;flex-direction:row;gap:12px;padding:16px;align-items:center;justify-content:space-between",
        ],
    );
    let e = css(frame(
        json!({"layout": "column", "gap": 0, "padding": 0, "align": "end"}),
        vec![],
    ));
    let r = rule(&e, "f").unwrap();
    assert!(
        r.contains("flex-direction:column") && r.contains("align-items:flex-end"),
        "{r}"
    );
    assert!(
        !r.contains("gap:") && !r.contains("padding:"),
        "zero gap and padding are left out: {r}"
    );
}

#[test]
fn padding_per_side_is_four_values_and_a_side_follows_padding() {
    let e = css(frame(
        json!({"layout": "row", "padding": 8, "padT": 1, "padL": 4}),
        vec![],
    ));
    has(rule(&e, "f"), &["padding:1px 8px 8px 4px"]);
}

#[test]
fn rule_g2_justify_center_and_end_are_safe() {
    for (j, css_value) in [
        ("start", "flex-start"),
        ("center", "safe center"),
        ("end", "safe flex-end"),
    ] {
        let e = css(frame(json!({"layout": "row", "justify": j}), vec![]));
        has(rule(&e, "f"), &[&format!("justify-content:{css_value}")]);
    }
}

#[test]
fn clip_is_overflow_hidden_on_a_frame() {
    let e = css(frame(json!({"clip": true}), vec![]));
    has(rule(&e, "f"), &["overflow:hidden"]);
    let e = css(frame(json!({"clip": false}), vec![]));
    assert!(!rule(&e, "f").unwrap().contains("overflow"));
}

#[test]
fn margin_belongs_to_a_child_of_a_layout_frame_and_an_absolute_child_has_none() {
    let m = json!({"id": "m", "type": "rect", "marT": 1, "marR": 2, "marB": 3, "marL": 4});
    let e = css(frame(json!({"layout": "row"}), vec![m.clone()]));
    has(rule(&e, "m"), &["margin:1px 2px 3px 4px"]);
    let e = css(frame(json!({}), vec![m.clone()]));
    assert!(
        !rule(&e, "m").unwrap().contains("margin"),
        "a frame without layout ignores it"
    );
    let mut abs = m;
    abs["absolute"] = json!(true);
    let e = css(frame(json!({"layout": "row"}), vec![abs]));
    assert!(
        !rule(&e, "m").unwrap().contains("margin"),
        "margins do not move an absolute child"
    );
}

#[test]
fn an_absolute_child_of_a_layout_frame_sits_at_its_own_place_in_a_positioned_parent() {
    let a = json!({"id": "a", "type": "rect", "absolute": true, "x": 7, "y": 9});
    let g = json!({"id": "g", "type": "frame", "layout": "row"});
    let mut list = frame(json!({"layout": "row"}), vec![g]);
    let mut a = a;
    a["parentId"] = json!("g");
    a["pos"] = json!("a");
    list.as_array_mut().unwrap().push(a);
    let e = css(list);
    has(rule(&e, "a"), &["position:absolute;left:7px;top:9px"]);
    has(rule(&e, "g"), &["position:relative"]);
}

#[test]
fn a_frame_without_layout_that_has_children_is_positioned_when_nested() {
    let g = json!({"id": "g", "type": "frame"});
    let mut list = frame(
        json!({"layout": "row"}),
        vec![g, json!({"id": "x", "type": "rect"})],
    );
    list.as_array_mut()
        .unwrap()
        .push(json!({"id": "k", "type": "rect", "parentId": "g", "pos": "a"}));
    let e = css(list);
    has(rule(&e, "g"), &["position:relative"]);
}
