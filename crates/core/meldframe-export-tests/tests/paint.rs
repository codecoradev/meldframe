//! E4.1: typography and paint, one test per row of sections 4.2 and 4.3 of the fidelity contract that the model has
//! (fill, radius, opacity, gradient, border, shadows, image, font size and weight, text align, shader).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod support;
use serde_json::json;
use support::*;

fn r(extra: serde_json::Value) -> serde_json::Value {
    let mut n = json!({"id": "r", "type": "rect", "w": 100, "h": 50});
    n.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    n
}

fn rule_of(list: serde_json::Value) -> String {
    rule(&css(list), "r").unwrap().to_owned()
}

#[test]
fn fill_is_the_background_colour_and_none_is_no_background() {
    let s = rule_of(frame(json!({}), vec![r(json!({"fill": "#336699"}))]));
    assert!(s.contains("background-color:#336699"), "{s}");
    let s = rule_of(frame(json!({}), vec![r(json!({"fill": "none"}))]));
    assert!(!s.contains("background"), "{s}");
    // The canvas drops the alpha of a fill, so the export does too: one render model.
    let s = rule_of(frame(json!({}), vec![r(json!({"fill": "#33669980"}))]));
    assert!(
        s.contains("background-color:#336699;") || s.ends_with("background-color:#336699"),
        "{s}"
    );
}

#[test]
fn the_fill_of_a_text_is_its_colour() {
    let t = json!({"id": "t", "type": "text", "text": "x", "fill": "#ff0000"});
    let e = css(frame(json!({}), vec![t]));
    assert!(rule(&e, "t").unwrap().contains("color:#ff0000"));
    assert!(!rule(&e, "t").unwrap().contains("background"));
}

#[test]
fn radius_is_a_border_radius_and_an_ellipse_is_fifty_percent() {
    assert!(
        rule_of(frame(json!({}), vec![r(json!({"radius": 12}))])).contains("border-radius:12px")
    );
    assert!(!rule_of(frame(json!({}), vec![r(json!({}))])).contains("border-radius"));
    let e = css(frame(
        json!({}),
        vec![json!({"id": "e", "type": "ellipse"})],
    ));
    assert!(rule(&e, "e").unwrap().contains("border-radius:50%"));
}

#[test]
fn opacity_below_one_is_exported_and_one_is_not() {
    assert!(rule_of(frame(json!({}), vec![r(json!({"opacity": 0.5}))])).contains("opacity:0.5"));
    assert!(!rule_of(frame(json!({}), vec![r(json!({"opacity": 1}))])).contains("opacity"));
}

#[test]
fn a_linear_gradient_keeps_the_css_angle_and_its_stops() {
    let g = json!({"kind": "linear", "angle": 90, "stops": [{"pos": 0, "color": "#ff0000"}, {"pos": 0.3, "color": "#00ff0080"}, {"pos": 1, "color": "#0000ff"}]});
    let s = rule_of(frame(json!({}), vec![r(json!({"gradient": g}))]));
    assert!(
        s.contains("background-image:linear-gradient(90deg,#ff0000 0%,#00ff0080 30%,#0000ff 100%)"),
        "{s}"
    );
    assert!(
        !s.contains("background-size"),
        "a gradient alone needs no size: {s}"
    );
}

#[test]
fn a_radial_gradient_reaches_the_farthest_corner() {
    let g = json!({"kind": "radial", "stops": [{"pos": 0, "color": "#ffffff"}, {"pos": 1, "color": "#000000"}]});
    let s = rule_of(frame(json!({}), vec![r(json!({"gradient": g}))]));
    assert!(
        s.contains("radial-gradient(circle farthest-corner,#ffffff 0%,#000000 100%)"),
        "{s}"
    );
}

#[test]
fn an_image_is_over_the_gradient_and_the_fill_with_its_fit() {
    let asset = "a".repeat(64);
    for (fit, size) in [
        ("cover", "cover"),
        ("contain", "contain"),
        ("fill", "100% 100%"),
    ] {
        let s = rule_of(frame(
            json!({}),
            vec![r(json!({"image": {"assetId": asset, "fit": fit}}))],
        ));
        assert!(
            s.contains(&format!("background-image:url(assets/{asset})")),
            "{s}"
        );
        assert!(
            s.contains(&format!(
                "background-size:{size};background-position:center;background-repeat:no-repeat"
            )),
            "{s}"
        );
        assert!(
            s.contains("background-color:"),
            "the fill shows around a contained image: {s}"
        );
    }
    let g = json!({"kind": "linear", "angle": 0, "stops": [{"pos": 0, "color": "#ff0000"}, {"pos": 1, "color": "#0000ff"}]});
    let e = css(frame(
        json!({}),
        vec![r(json!({"gradient": g, "image": {"assetId": asset}}))],
    ));
    let s = rule(&e, "r").unwrap();
    assert!(
        s.contains(&format!(
            "background-image:url(assets/{asset}),linear-gradient(0deg"
        )),
        "{s}"
    );
    assert!(s.contains("background-size:cover,100% 100%"), "{s}");
    assert_eq!(e.report.assets, vec![asset]);
}

#[test]
fn shadows_are_listed_top_first_as_css_wants_them() {
    let w = json!([{"x": 1, "y": 2, "blur": 3, "spread": 4, "color": "#ff0000"}, {"x": 0, "y": 0, "blur": 0, "spread": 10, "color": "#0000ff"}]);
    let s = rule_of(frame(json!({}), vec![r(json!({"shadows": w}))]));
    // Model: the first is at the bottom. CSS: the first is on top, so the list is reversed (probe: last in list on top).
    assert!(
        s.contains("box-shadow:0px 0px 0px 10px #0000ff,1px 2px 3px 4px #ff0000"),
        "{s}"
    );
}

#[test]
fn a_border_is_paint_only_on_after_and_never_a_css_border() {
    for (pos, inset) in [("inside", "0px"), ("center", "-1.5px"), ("outside", "-3px")] {
        let b = json!({"width": 3, "color": "#ff0000", "position": pos});
        let e = css(frame(
            json!({"clip": false, "layout": "row"}),
            vec![r(json!({"border": b}))],
        ));
        let s = rule(&e, "r").unwrap();
        assert!(s.contains("position:relative"), "{s}");
        assert!(
            !s.contains("border:"),
            "a real border would move the content: {s}"
        );
        let a = after(&e, "r").unwrap();
        assert!(a.contains(&format!("content:\"\";position:absolute;pointer-events:none;inset:{inset};border:3px solid #ff0000")), "{a}");
    }
}

#[test]
fn the_border_follows_the_radius_and_grows_with_its_position() {
    let b = |pos: &str| json!({"width": 4, "color": "#000000", "position": pos});
    let e = css(frame(
        json!({}),
        vec![r(json!({"radius": 10, "border": b("outside")}))],
    ));
    assert!(
        after(&e, "r").unwrap().contains("border-radius:14px"),
        "{}",
        after(&e, "r").unwrap()
    );
    let e = css(frame(
        json!({}),
        vec![r(json!({"radius": 10, "border": b("inside")}))],
    ));
    assert!(after(&e, "r").unwrap().contains("border-radius:10px"));
    // A radius above half the size is held to it, as the canvas does.
    let e = css(frame(
        json!({}),
        vec![r(json!({"radius": 99, "border": b("inside")}))],
    ));
    assert!(after(&e, "r").unwrap().contains("border-radius:25px"));
    let e = css(frame(
        json!({}),
        vec![json!({"id": "e", "type": "ellipse", "border": b("inside")})],
    ));
    assert!(after(&e, "e").unwrap().contains("border-radius:50%"));
}

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
fn the_tailwind_form_of_the_paint() {
    let w = json!([{"x": 1, "y": 2, "blur": 3, "spread": 4, "color": "#ff0000"}]);
    let b = json!({"width": 3, "color": "#ff0000", "position": "center"});
    let n = r(json!({"fill": "#336699", "radius": 8, "opacity": 0.5, "shadows": w, "border": b}));
    let e = tailwind(frame(json!({"clip": false, "layout": "row"}), vec![n]));
    let c = classes(&e, "r");
    for want in [
        "bg-[color:#336699]",
        "rounded-[8px]",
        "opacity-[0.5]",
        "shadow-[1px_2px_3px_4px_#ff0000]",
        "relative",
        "after:content-['']",
        "after:absolute",
        "after:pointer-events-none",
        "after:inset-[-1.5px]",
        "after:[border:3px_solid_#ff0000]",
        "after:rounded-[9.5px]",
    ] {
        // Radius 8 on a 100 x 50 rect grows by half the border (3 / 2) on the edge: 9.5.
        assert!(c.contains(&want), "{want} is not in {c:?}");
    }
    let t = json!({"id": "t", "type": "text", "text": "x", "fontWeight": 700, "fill": "#ff0000"});
    let e = tailwind(frame(json!({}), vec![t]));
    let c = classes(&e, "t");
    assert!(
        c.contains(&"font-[700]") && c.contains(&"text-[color:#ff0000]"),
        "{c:?}"
    );
    let asset = "b".repeat(64);
    let e = tailwind(frame(
        json!({}),
        vec![r(json!({"image": {"assetId": asset, "fit": "fill"}}))],
    ));
    let c = classes(&e, "r");
    assert!(
        c.iter()
            .any(|x| x.starts_with("[background-image:url(assets/")),
        "{c:?}"
    );
    assert!(c.contains(&"[background-size:100%_100%]"), "{c:?}");
}

#[test]
fn a_shader_is_approximated_by_a_gradient_and_reported() {
    let s = json!({"id": "s", "type": "shader", "fill": "#112233", "fill2": "#445566"});
    let e = css(frame(json!({}), vec![s]));
    assert!(
        rule(&e, "s")
            .unwrap()
            .contains("background-image:linear-gradient(135deg,#112233,#445566)")
    );
    let f = e
        .report
        .unsupported
        .iter()
        .find(|f| f.code == "shader_approximated")
        .expect("reported");
    assert_eq!(f.node.as_deref(), Some("s"));
}

#[test]
fn what_a_text_cannot_paint_is_dropped_and_reported() {
    let t = json!({"id": "t", "type": "text", "text": "x", "border": {"width": 2}, "shadows": [{"blur": 4}]});
    let e = css(frame(json!({}), vec![t]));
    assert!(!rule(&e, "t").unwrap().contains("box-shadow"));
    assert!(after(&e, "t").is_none());
    let on_text: Vec<_> = e
        .report
        .unsupported
        .iter()
        .filter(|f| f.code == "on_text")
        .collect();
    assert_eq!(on_text.len(), 2, "{:?}", e.report);
}

#[test]
fn a_border_on_a_frame_that_clips_is_reported_unless_it_is_inside() {
    let b = |pos: &str| json!({"width": 2, "position": pos});
    let e = css(frame(json!({"border": b("outside")}), vec![]));
    assert!(
        e.report
            .unsupported
            .iter()
            .any(|f| f.code == "border_clipped")
    );
    let e = css(frame(json!({"border": b("inside")}), vec![]));
    assert!(e.report.unsupported.is_empty(), "{:?}", e.report);
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
