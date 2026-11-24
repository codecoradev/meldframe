//! E3.1: `resolve` turns nodes into the boxes the canvas paints.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_protocol::{GradientKind, ImageFit, Node, Variables};
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
