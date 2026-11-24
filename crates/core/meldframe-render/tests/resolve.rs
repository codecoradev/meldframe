//! E3.1: `resolve` turns nodes into the boxes the canvas paints.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_protocol::{BorderPosition, GradientKind, ImageFit, Node, Variables};
use meldframe_render::{ModeChoice, Paint, PaintedBox, resolve, resolve_with};
use serde_json::{Value, json};

fn node(v: Value) -> Node {
    serde_json::from_value(v).unwrap()
}

fn one(v: Value) -> PaintedBox {
    let boxes = resolve(&[node(v)]);
    assert_eq!(boxes.len(), 1);
    boxes.into_iter().next().unwrap()
}

#[test]
fn parents_come_before_children_and_siblings_by_position_then_id() {
    let nodes = [
        node(json!({"id": "c2", "type": "rect", "parentId": "p", "pos": "b"})),
        node(json!({"id": "c1", "type": "rect", "parentId": "p", "pos": "a"})),
        node(json!({"id": "p", "type": "frame", "pos": "a"})),
        node(json!({"id": "t", "type": "rect", "parentId": "p", "pos": "b"})),
    ];
    let ids: Vec<_> = resolve(&nodes).into_iter().map(|b| b.id).collect();
    assert_eq!(ids, ["p", "c1", "c2", "t"]);
}

#[test]
fn a_node_without_position_keeps_list_order_before_positioned_ones() {
    let nodes = [
        node(json!({"id": "z", "type": "rect"})),
        node(json!({"id": "a", "type": "rect"})),
        node(json!({"id": "m", "type": "rect", "pos": "a"})),
    ];
    let ids: Vec<_> = resolve(&nodes).into_iter().map(|b| b.id).collect();
    assert_eq!(ids, ["z", "a", "m"]);
}

#[test]
fn a_node_whose_parent_is_missing_is_a_root_and_a_cycle_stays_visible() {
    let nodes = [
        node(json!({"id": "o", "type": "rect", "parentId": "gone"})),
        node(json!({"id": "a", "type": "frame", "parentId": "b"})),
        node(json!({"id": "b", "type": "frame", "parentId": "a"})),
    ];
    let boxes = resolve(&nodes);
    assert_eq!(boxes.len(), 3);
    assert_eq!(boxes[0].parent, None);
    assert!(boxes.iter().all(|b| b.parent.as_deref() != Some("gone")));
}

#[test]
fn fill_is_a_number_and_none_for_no_fill_or_a_bad_colour() {
    assert_eq!(
        one(json!({"id": "n", "type": "rect", "fill": "#ff8000"})).fill,
        Some(0xff8000)
    );
    assert_eq!(
        one(json!({"id": "n", "type": "rect", "fill": "#f80"})).fill,
        Some(0xff8800)
    );
    assert_eq!(
        one(json!({"id": "n", "type": "rect", "fill": "#11223344"})).fill,
        Some(0x112233)
    );
    assert_eq!(
        one(json!({"id": "n", "type": "rect", "fill": "none"})).fill,
        None
    );
    assert_eq!(
        one(json!({"id": "n", "type": "rect", "fill": "red"})).fill,
        None
    );
}

#[test]
fn the_alpha_of_a_fill_is_kept_next_to_its_colour() {
    // #rrggbbaa and #rgba: the colour without its alpha, and the alpha as a fraction of 255 (issue 234: it was dropped,
    // and a `#rgba` fill was no fill at all).
    let b = one(json!({"id": "n", "type": "rect", "fill": "#ff000080"}));
    assert_eq!((b.fill, b.fill_alpha), (Some(0xff0000), 128.0 / 255.0));
    let b = one(json!({"id": "n", "type": "rect", "fill": "#2563eb40"}));
    assert_eq!((b.fill, b.fill_alpha), (Some(0x2563eb), 64.0 / 255.0));
    let b = one(json!({"id": "n", "type": "rect", "fill": "#00000000"}));
    assert_eq!((b.fill, b.fill_alpha), (Some(0), 0.0));
    let b = one(json!({"id": "n", "type": "rect", "fill": "#f008"}));
    assert_eq!(
        (b.fill, b.fill_alpha),
        (Some(0xff0000), 0x88 as f64 / 255.0)
    );
    // Opaque in every form that has no alpha, and in the forms with a full one.
    for (css, rgb) in [
        ("#ff8000", 0xff8000),
        ("#f80", 0xff8800),
        ("#ff8000ff", 0xff8000),
        ("#f80f", 0xff8800),
    ] {
        let b = one(json!({"id": "n", "type": "rect", "fill": css}));
        assert_eq!((b.fill, b.fill_alpha), (Some(rgb), 1.0), "{css}");
    }
    // No fill, or a bad colour: nothing is painted, and the alpha says nothing (it stays 1).
    for css in ["none", "red", "#12", "#12345"] {
        let b = one(json!({"id": "n", "type": "rect", "fill": css}));
        assert_eq!((b.fill, b.fill_alpha), (None, 1.0), "{css}");
    }
}

#[test]
fn the_alpha_of_a_bound_fill_is_the_alpha_of_the_variable_value() {
    let nodes = [node(
        json!({"id": "n", "type": "rect", "fill": {"var": "v-glass"}}),
    )];
    let vars: Variables = serde_json::from_value(json!({
        "collections": [{"id": "c", "name": "brand", "modes": [{"id": "m", "name": "light"}]}],
        "variables": [{"id": "v-glass", "collection": "c", "name": "color/glass", "type": "color", "values": {"m": "#11223380"}}]
    }))
    .unwrap();
    let boxes = resolve_with(&nodes, &vars, &ModeChoice::default());
    assert_eq!(boxes[0].fill, Some(0x112233));
    assert_eq!(boxes[0].fill_alpha, 128.0 / 255.0);
}

#[test]
fn an_ellipse_is_a_pill_and_a_shader_its_plain_box() {
    let e = one(json!({"id": "n", "type": "ellipse", "w": 80, "h": 40, "radius": 3}));
    assert_eq!((e.paint, e.radius), (Paint::Rect, 20.0));
    let s = one(json!({"id": "n", "type": "shader", "radius": 5}));
    assert_eq!((s.paint, s.radius), (Paint::Rect, 5.0));
    assert_eq!(one(json!({"id": "n", "type": "frame"})).paint, Paint::Frame);
    let t = one(json!({"id": "n", "type": "text", "text": "hi", "fontSize": 20}));
    assert_eq!(
        (t.paint, t.text.as_str(), t.font_size),
        (Paint::Text, "hi", 20.0)
    );
}

#[test]
fn opacity_defaults_to_one() {
    assert_eq!(one(json!({"id": "n", "type": "rect"})).opacity, 1.0);
    assert_eq!(
        one(json!({"id": "n", "type": "rect", "opacity": 0.25})).opacity,
        0.25
    );
}

#[test]
fn opacity_that_is_not_below_one_or_not_a_number_is_one() {
    // The canvas keeps an opacity only when it is a number below 1 (`scene.ts`).
    for o in [json!(1.0), json!(1.5)] {
        let b = one(json!({"id": "n", "type": "rect", "opacity": o}));
        assert_eq!(b.opacity, 1.0, "opacity {o}");
    }
    let mut n: Node = serde_json::from_value(json!({"id": "n", "type": "rect"})).unwrap();
    n.opacity = f64::NAN.into();
    assert_eq!(resolve(&[n])[0].opacity, 1.0, "opacity NaN");
}

#[test]
fn gradient_and_image_are_painted_except_on_text_or_with_a_bad_stop() {
    let g = json!({"kind": "radial", "angle": 90, "stops": [{"pos": 0, "color": "#fff"}, {"pos": 1, "color": "#000000ff"}]});
    let b = one(
        json!({"id": "n", "type": "rect", "gradient": g, "image": {"assetId": "abc", "fit": "contain"}}),
    );
    let grad = b.gradient.unwrap();
    assert_eq!(
        (grad.kind, grad.angle, grad.stops.len()),
        (GradientKind::Radial, 90.0, 2)
    );
    assert_eq!(grad.stops[1], (1.0, "#000000ff".to_string()));
    let img = b.image.unwrap();
    assert_eq!((img.asset_id.as_str(), img.fit), ("abc", ImageFit::Contain));

    let t = one(json!({"id": "n", "type": "text", "gradient": g, "image": {"assetId": "abc"}}));
    assert!(t.gradient.is_none() && t.image.is_none());

    let bad = json!({"stops": [{"pos": 0, "color": "red"}, {"pos": 1, "color": "#000"}]});
    assert!(
        one(json!({"id": "n", "type": "rect", "gradient": bad}))
            .gradient
            .is_none()
    );
}

#[test]
fn a_border_is_painted_with_its_width_colour_alpha_and_position() {
    let b = one(
        json!({"id": "n", "type": "rect", "border": {"width": 6, "color": "#ff000080", "position": "outside"}}),
    );
    let border = b.border.expect("a border with a width is painted");
    assert_eq!(
        (border.width, border.color, border.position),
        (6.0, 0xff0000, BorderPosition::Outside)
    );
    assert_eq!(border.alpha, f64::from(0x80u8) / 255.0);
    // The default position is inside and the default colour opaque black.
    let d = one(json!({"id": "n", "type": "frame", "border": {"width": 2}}))
        .border
        .unwrap();
    assert_eq!(
        (d.color, d.alpha, d.position),
        (0, 1.0, BorderPosition::Inside)
    );
    let short = one(json!({"id": "n", "type": "rect", "border": {"width": 1, "color": "#f80"}}))
        .border
        .unwrap();
    assert_eq!(short.color, 0xff8800);
}

#[test]
fn no_border_is_painted_for_no_width_a_text_or_a_colour_that_is_not_hex() {
    for v in [
        json!({"id": "n", "type": "rect"}),
        json!({"id": "n", "type": "rect", "border": {"width": 0, "color": "#ff0000"}}),
        json!({"id": "n", "type": "text", "text": "hi", "border": {"width": 4}}),
        json!({"id": "n", "type": "rect", "border": {"width": 4, "color": "red"}}),
    ] {
        assert!(one(v.clone()).border.is_none(), "{v}");
    }
    // An ellipse and a shader take a border like a rectangle: the canvas draws them as rounded boxes.
    for kind in ["ellipse", "shader", "frame"] {
        assert!(
            one(json!({"id": "n", "type": kind, "border": {"width": 3}}))
                .border
                .is_some(),
            "{kind}"
        );
    }
}

#[test]
fn shadows_are_painted_in_model_order_with_their_numbers_colour_and_alpha() {
    let b = one(json!({"id": "n", "type": "rect", "shadows": [
        {"x": 1, "y": 2, "blur": 3, "spread": 4, "color": "#ff000080"},
        {"x": -5, "color": "#f80"},
        {},
    ]}));
    let s = &b.shadows;
    assert_eq!(s.len(), 3);
    assert_eq!(
        (s[0].x, s[0].y, s[0].blur, s[0].spread),
        (1.0, 2.0, 3.0, 4.0)
    );
    assert_eq!(
        (s[0].color, s[0].alpha),
        (0xff0000, f64::from(0x80u8) / 255.0)
    );
    assert_eq!(
        (s[1].x, s[1].blur, s[1].color, s[1].alpha),
        (-5.0, 0.0, 0xff8800, 1.0)
    );
    // The default shadow is #00000040: black at a quarter.
    assert_eq!((s[2].color, s[2].alpha), (0, f64::from(0x40u8) / 255.0));
    assert!(one(json!({"id": "n", "type": "frame"})).shadows.is_empty());
}

#[test]
fn no_shadow_is_painted_for_a_text_a_colour_that_is_not_hex_or_a_number_that_is_not_finite() {
    let text = one(json!({"id": "n", "type": "text", "text": "hi", "shadows": [{"blur": 4}]}));
    assert!(text.shadows.is_empty());
    let mut n = node(json!({"id": "n", "type": "rect", "shadows": [
        {"color": "red"}, {"blur": 2}, {"x": 1}, {"y": 1}, {"spread": 1}, {"color": "#12"},
    ]}));
    n.shadows[2].x = f64::NAN;
    n.shadows[3].y = f64::INFINITY;
    n.shadows[4].spread = f64::NEG_INFINITY;
    let kept = &resolve(&[n])[0].shadows;
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(kept[0].blur, 2.0);
}

#[test]
fn a_blur_below_zero_is_a_hard_edge() {
    let mut n = node(json!({"id": "n", "type": "rect", "shadows": [{"blur": 4}]}));
    n.shadows[0].blur = -3.0;
    assert_eq!(resolve(&[n])[0].shadows[0].blur, 0.0);
}

#[test]
fn an_ellipse_and_a_shader_take_shadows_like_a_rectangle() {
    for kind in ["ellipse", "shader", "frame"] {
        let n = json!({"id": "n", "type": kind, "shadows": [{"y": 4, "blur": 8}]});
        assert_eq!(one(n).shadows.len(), 1, "{kind}");
    }
}
