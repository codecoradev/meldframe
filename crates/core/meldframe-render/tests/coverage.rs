//! E3.1: the coverage test. Every node field is in the table; a field a consumer renders changes
//! the painted boxes, and a field marked unsupported does not; an unsupported mark has a reason.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_layout::StubMeasurer;
use meldframe_protocol::Variables;
use meldframe_protocol::{Gradient, GradientStop, ImageFill, Node, NodeKind};
use meldframe_render::coverage::{FROM_LAYOUT, NODE_FIELDS, Support};
use meldframe_render::{ModeChoice, PaintedBox, resolve, resolve_laid_out};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// A node with every optional field present, so that its JSON lists every field name.
fn full_node() -> Node {
    let mut n: Node = serde_json::from_value(json!({"id": "n", "type": "rect"})).unwrap();
    n.pos = "a".into();
    n.pad_t = Some(1.0);
    n.pad_r = Some(1.0);
    n.pad_b = Some(1.0);
    n.pad_l = Some(1.0);
    n.gradient = Some(Gradient {
        kind: Default::default(),
        angle: 0.0,
        stops: vec![],
    });
    n.image = Some(ImageFill {
        asset_id: "a".into(),
        fit: Default::default(),
    });
    n
}

#[test]
fn the_table_lists_exactly_the_fields_of_a_node() {
    let Value::Object(map) = serde_json::to_value(full_node()).unwrap() else {
        panic!("a node is an object");
    };
    let fields: BTreeSet<&str> = map.keys().map(String::as_str).collect();
    let listed: BTreeSet<&str> = NODE_FIELDS.iter().map(|f| f.field).collect();
    assert_eq!(listed.len(), NODE_FIELDS.len(), "a field is listed twice");
    let missing: Vec<_> = fields.difference(&listed).collect();
    let stale: Vec<_> = listed.difference(&fields).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "node fields missing from the table: {missing:?}; entries for fields that do not exist: {stale:?}"
    );
}

/// A frame with a rectangle, a text and a shader under it, so that every field has a node that shows
/// it: the frame is a parent with children (layout fields), the shader node shows `shader`.
fn base() -> Vec<Node> {
    let n = |v: Value| -> Node { serde_json::from_value(v).unwrap() };
    vec![
        n(json!({"id": "p", "type": "frame"})),
        n(json!({"id": "r", "type": "rect", "parentId": "p", "pos": "b"})),
        n(json!({"id": "t", "type": "text", "parentId": "p", "pos": "a", "text": "x"})),
        n(json!({"id": "s", "type": "shader", "parentId": "p", "pos": "c"})),
    ]
}

/// Changes `field` on the node at `i` of `base` (0 the frame, 1 the rectangle, 2 the text, 3 the
/// shader) to a different value.
fn change(field: &str, i: usize, n: &mut Node) {
    let stop = |pos: f64, color: &str| GradientStop {
        pos,
        color: color.into(),
    };
    match field {
        "id" => n.id.push('2'),
        "name" => n.name = "other".into(),
        "type" => n.kind = NodeKind::Frame,
        "parentId" => n.parent_id = None,
        "pos" => n.pos = if i == 1 { "a" } else { "b" }.into(),
        "x" => n.x += 7.0,
        "y" => n.y += 7.0,
        "w" => n.w += 7.0,
        "h" => n.h += 7.0,
        "sw" => n.sw = meldframe_protocol::SizeMode::Fill,
        "sh" => n.sh = meldframe_protocol::SizeMode::Fill,
        "fill" => n.fill = "#112233".into(),
        "fill2" => n.fill2 = "#445566".into(),
        "radius" => n.radius = 3.0.into(),
        "layout" => n.layout = meldframe_protocol::LayoutMode::Row,
        "gap" => n.gap += 3.0,
        "padding" => n.padding += 3.0,
        "padT" => n.pad_t = Some(9.0),
        "padR" => n.pad_r = Some(9.0),
        "padB" => n.pad_b = Some(9.0),
        "padL" => n.pad_l = Some(9.0),
        "align" => n.align = meldframe_protocol::Align::End,
        "justify" => n.justify = meldframe_protocol::Justify::End,
        "clip" => n.clip = !n.clip,
        "text" => n.text = "changed".into(),
        "fontSize" => n.font_size += 3.0,
        "shader" => n.shader = meldframe_protocol::ShaderPreset::Waves,
        "fontWeight" => n.font_weight = 700,
        "textAlign" => n.text_align = meldframe_protocol::TextAlign::Center,
        "border" => n.border.width = 3.0,
        "shadows" => n.shadows = vec![serde_json::from_value(json!({"blur": 4})).unwrap()],
        "marT" => n.mar_t = 4.0,
        "marR" => n.mar_r = 4.0,
        "marB" => n.mar_b = 4.0,
        "marL" => n.mar_l = 4.0,
        "absolute" => n.absolute = true,
        "opacity" => n.opacity = 0.5.into(),
        "gradient" => {
            n.gradient = Some(Gradient {
                kind: Default::default(),
                angle: 90.0,
                stops: vec![stop(0.0, "#000000"), stop(1.0, "#ffffff")],
            })
        }
        "image" => {
            n.image = Some(ImageFill {
                asset_id: "asset".into(),
                fit: Default::default(),
            })
        }
        other => panic!("no change is defined for the node field `{other}`: add one to this test"),
    }
}

#[test]
fn a_rendered_field_changes_the_painted_boxes_and_an_unsupported_one_does_not() {
    let before = resolve(&base());
    assert_eq!(before.len(), 4);
    for f in NODE_FIELDS {
        let mut nodes = base();
        // Every node, the parent frame included: a layout field only shows on a node that has children.
        for (i, n) in nodes.iter_mut().enumerate() {
            change(f.field, i, n);
        }
        let moved = resolve(&nodes) != before;
        match f.canvas {
            Support::Rendered => assert!(
                moved,
                "`{}` is marked as rendered by the canvas but changing it paints nothing different",
                f.field
            ),
            Support::Unsupported(_) => assert!(
                !moved,
                "`{}` is marked unsupported by the canvas but changing it changes the painted boxes: mark it rendered",
                f.field
            ),
        }
    }
}

#[test]
fn a_field_one_consumer_renders_and_the_other_does_not_is_marked_unsupported_with_a_reason() {
    for f in NODE_FIELDS {
        for side in [f.canvas, f.export] {
            if let Support::Unsupported(why) = side {
                assert!(
                    why.trim().len() > 10,
                    "`{}`: an unsupported mark needs a reason",
                    f.field
                );
            }
        }
    }
}

fn laid_out(nodes: &[Node]) -> Vec<PaintedBox> {
    resolve_laid_out(
        nodes,
        &Variables::default(),
        &ModeChoice::default(),
        &StubMeasurer,
    )
}

/// A column frame with two children, in two forms so that every layout field has room to show. Fixed: the frame is
/// bigger than what is in it, so the children are narrower than it (align) and it is taller than they are (justify).
/// Hug: the frame is as big as its content, so the padding on the far sides and the margins give it its size.
fn column(hug: bool) -> Vec<Node> {
    let n = |v: Value| -> Node { serde_json::from_value(v).unwrap() };
    let frame = if hug {
        json!({"id": "p", "type": "frame", "layout": "column", "sw": "hug", "sh": "hug"})
    } else {
        json!({"id": "p", "type": "frame", "layout": "column", "w": 200, "h": 200})
    };
    vec![
        n(frame),
        n(json!({"id": "r", "type": "rect", "parentId": "p", "pos": "a", "w": 20, "h": 20})),
        n(json!({"id": "s", "type": "rect", "parentId": "p", "pos": "b", "w": 20, "h": 20})),
    ]
}

/// The table says the canvas does not render the layout fields because `resolve` reads the stored box. That stays
/// true of `resolve` (the test above) and so of the default canvas; this test says what the flag adds: with
/// `resolve_laid_out`, which the canvas reads when the host turns `layout_on_canvas` on, every field the table marks
/// as an input of layout does change what is painted.
#[test]
fn the_layout_fields_the_canvas_does_not_render_by_default_are_rendered_by_resolve_laid_out() {
    let rows: Vec<_> = NODE_FIELDS
        .iter()
        .filter(|f| f.canvas == FROM_LAYOUT)
        .collect();
    assert_eq!(rows.len(), 16, "the layout inputs of the table");
    for f in rows {
        let mut shown = false;
        for hug in [false, true] {
            let mut nodes = column(hug);
            for (i, n) in nodes.iter_mut().enumerate() {
                change(f.field, i, n);
            }
            shown |= laid_out(&nodes) != laid_out(&column(hug));
            assert!(
                resolve(&nodes) == resolve(&column(hug)),
                "`{}` is marked unsupported by the canvas: resolve must still ignore it",
                f.field
            );
        }
        assert!(
            shown,
            "`{}` is an input of layout but changing it paints nothing different through resolve_laid_out",
            f.field
        );
    }
}
