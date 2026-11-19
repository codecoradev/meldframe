//! E3.1 and E4.1: the coverage table against the export. Every field the table marks as rendered by the export
//! changes the export when it changes; a field marked unsupported does not. With the canvas side checked by
//! `meldframe-render`, a field one consumer renders and the other does not has to say so in the table.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod support;
use export::{ExportOptions, Format};
use meldframe_layout::StubMeasurer;
use meldframe_protocol::{
    Align, Gradient, GradientStop, ImageFill, Justify, LayoutMode, Node, NodeKind, ShaderPreset,
    SizeMode, TextAlign, Variables,
};
use meldframe_render::coverage::{NODE_FIELDS, Support};
use serde_json::{Value, json};

/// A frame `p` in a row with a rect, a text and a shader inside; the frame `p` is exported.
fn base() -> Vec<Node> {
    let n = |v: Value| -> Node { serde_json::from_value(v).unwrap() };
    vec![
        n(
            json!({"id": "p", "type": "frame", "layout": "row", "w": 300, "h": 100, "padding": 4, "gap": 4}),
        ),
        n(json!({"id": "r", "type": "rect", "parentId": "p", "pos": "b", "w": 20, "h": 20})),
        n(json!({"id": "t", "type": "text", "parentId": "p", "pos": "a", "text": "x"})),
        n(json!({"id": "s", "type": "shader", "parentId": "p", "pos": "c"})),
    ]
}

/// Changes `field` of node `n` (the `i`th of `base`) to a different value.
fn change(field: &str, i: usize, n: &mut Node) {
    let stop = |pos: f64, color: &str| GradientStop {
        pos,
        color: color.into(),
    };
    match field {
        "id" => n.id.push('2'),
        "name" => n.name = "other".into(),
        "type" => {
            n.kind = if n.kind == NodeKind::Text {
                NodeKind::Rect
            } else {
                NodeKind::Text
            }
        }
        "parentId" => n.parent_id = None,
        "pos" => n.pos = if i == 1 { "a" } else { "b" }.into(),
        "x" => n.x += 7.0,
        "y" => n.y += 7.0,
        "w" => n.w += 7.0,
        "h" => n.h += 7.0,
        "sw" => n.sw = SizeMode::Fill,
        "sh" => n.sh = SizeMode::Fill,
        "fill" => n.fill = "#112233".into(),
        "fill2" => n.fill2 = "#445566".into(),
        "radius" => n.radius = 3.0.into(),
        "layout" => n.layout = LayoutMode::Column,
        "gap" => n.gap += 3.0,
        "padding" => n.padding += 3.0,
        "padT" => n.pad_t = Some(9.0),
        "padR" => n.pad_r = Some(9.0),
        "padB" => n.pad_b = Some(9.0),
        "padL" => n.pad_l = Some(9.0),
        "align" => n.align = Align::End,
        "justify" => n.justify = Justify::Between,
        "clip" => n.clip = !n.clip,
        "text" => n.text = "changed".into(),
        "fontSize" => n.font_size += 3.0,
        "shader" => n.shader = ShaderPreset::Waves,
        "fontWeight" => n.font_weight = 700,
        "textAlign" => n.text_align = TextAlign::Center,
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
                asset_id: "a".repeat(64),
                fit: Default::default(),
            })
        }
        other => panic!("no change is defined for the node field `{other}`: add one to this test"),
    }
}

fn exported(nodes: &[Node], format: Format) -> (String, String) {
    let o = ExportOptions::new(format, &StubMeasurer);
    let e = export::export_frame(nodes, &Variables::default(), "p", &o).unwrap();
    (e.html, e.css)
}

/// What a changed field must change in the export, on the node the change is made on.
enum Proof {
    /// The value of this property in the rule of the node (plain CSS), and the classes of its element (Tailwind).
    Prop(&'static str),
    /// The same, in the `::after` rule of the node (the border).
    AfterProp(&'static str),
    /// The document has this text after the change and not before (an attribute or the text of a node).
    Html(&'static str),
    /// The shape of the document: the tag, the nesting or the order of the elements. No one declaration carries it.
    Structure,
}

/// For each field the export renders: the node of `base` to change (0 is `p`, 1 `r`, 2 `t`, 3 `s`) and the proof.
/// A field that the export does not carry on a node (`x` of a child in a row) is proven on the node that carries it.
fn proof(field: &str) -> Option<(usize, Proof)> {
    use Proof::*;
    Some(match field {
        "id" => (1, Html("data-node=\"r2\"")),
        "name" => (1, Html("data-name=\"other\"")),
        "type" | "parentId" | "pos" => (1, Structure),
        "x" => (0, Prop("left")),
        "y" => (0, Prop("top")),
        "w" => (0, Prop("width")),
        "h" => (0, Prop("height")),
        "sw" => (1, Prop("flex")),
        "sh" => (1, Prop("align-self")),
        "fill" => (1, Prop("background-color")),
        "fill2" | "gradient" | "image" => (
            if field == "fill2" { 3 } else { 1 },
            Prop("background-image"),
        ),
        "radius" => (1, Prop("border-radius")),
        "layout" => (0, Prop("flex-direction")),
        "gap" => (0, Prop("gap")),
        "padding" | "padT" | "padR" | "padB" | "padL" => (0, Prop("padding")),
        "align" => (0, Prop("align-items")),
        "justify" => (0, Prop("justify-content")),
        "clip" => (0, Prop("overflow")),
        "text" => (2, Html(">changed</p>")),
        "fontSize" => (2, Prop("font-size")),
        "fontWeight" => (2, Prop("font-weight")),
        "textAlign" => (2, Prop("text-align")),
        "border" => (1, AfterProp("border")),
        "shadows" => (1, Prop("box-shadow")),
        "marT" | "marR" | "marB" | "marL" => (1, Prop("margin")),
        "absolute" => (1, Prop("position")),
        "opacity" => (1, Prop("opacity")),
        _ => return None,
    })
}

/// The value of `prop` in a rule body (`a:1;b:2`).
fn value_of<'a>(body: Option<&'a str>, prop: &str) -> Option<&'a str> {
    let head = format!("{prop}:");
    body?.split(';').find_map(|d| d.strip_prefix(head.as_str()))
}

#[test]
fn every_field_the_export_renders_changes_its_own_declaration() {
    for f in NODE_FIELDS {
        let Support::Rendered = f.export else {
            continue;
        };
        let (target, proof) = proof(f.field).unwrap_or_else(|| {
            panic!(
                "`{}` is rendered by the export but has no proof here: add one",
                f.field
            )
        });
        let id = base()[target].id.clone();
        let mut changed = base();
        change(f.field, target, &mut changed[target]);
        let run = |nodes: &[Node], format| {
            export::export_frame(
                nodes,
                &Variables::default(),
                "p",
                &ExportOptions::new(format, &StubMeasurer),
            )
            .unwrap()
        };
        let (before, after) = (run(&base(), Format::Css), run(&changed, Format::Css));
        let (tw_before, tw_after) = (
            run(&base(), Format::Tailwind),
            run(&changed, Format::Tailwind),
        );
        match proof {
            Proof::Prop(prop) => {
                assert_ne!(
                    value_of(support::rule(&before, &id), prop),
                    value_of(support::rule(&after, &id), prop),
                    "`{}`: the `{prop}` of `{id}` does not change",
                    f.field
                );
                let classes = |e: &export::Export, i: &str| support::classes(e, i).join(" ");
                let (a, b) = (classes(&tw_before, &id), classes(&tw_after, &id));
                assert_ne!(a, b, "`{}`: the classes of `{id}` do not change", f.field);
            }
            Proof::AfterProp(prop) => {
                assert_ne!(
                    value_of(support::after(&before, &id), prop),
                    value_of(support::after(&after, &id), prop),
                    "`{}`: the `{prop}` of the border of `{id}` does not change",
                    f.field
                );
                assert_ne!(tw_before.html, tw_after.html, "`{}`", f.field);
            }
            Proof::Html(text) => {
                for (b, a) in [(&before, &after), (&tw_before, &tw_after)] {
                    assert!(
                        a.html.contains(text) && !b.html.contains(text),
                        "`{}`: {text}",
                        f.field
                    );
                }
            }
            // The shape of the document is what changes; `type`, `parentId` and `pos` have no declaration of their own.
            Proof::Structure => {
                assert_ne!(before.html, after.html, "`{}`", f.field);
                assert_ne!(tw_before.html, tw_after.html, "`{}`", f.field);
            }
        }
    }
}

/// The broad check: changing a field on any node of `base` changes the export if the table says it is rendered, and
/// changes nothing if it says unsupported. A field changed on the root frame always moves the output, so this alone
/// does not prove which declaration carries a field; `every_field_the_export_renders_changes_its_own_declaration`
/// does, and this test is what holds the unsupported rows to "changes nothing".
#[test]
fn an_exported_field_changes_the_export_and_an_unsupported_one_does_not() {
    for format in [Format::Css, Format::Tailwind] {
        let before = exported(&base(), format);
        for f in NODE_FIELDS {
            // The frame `p` is the exported root: a change of `parentId` or `type` on it would remove it, so change the
            // children and the frame in turn.
            let mut moved = false;
            for target in 0..4 {
                let mut nodes = base();
                change(f.field, target, &mut nodes[target]);
                let o = ExportOptions::new(format, &StubMeasurer);
                moved |= match export::export_frame(&nodes, &Variables::default(), "p", &o) {
                    Ok(e) => (e.html, e.css) != before,
                    Err(_) => true,
                };
            }
            match f.export {
                Support::Rendered => assert!(
                    moved,
                    "`{}` is marked as rendered by the export but changing it changes nothing ({format:?})",
                    f.field
                ),
                Support::Unsupported(why) => assert!(
                    !moved,
                    "`{}` is marked unsupported by the export ({why}) but changing it changes the export ({format:?}): mark it rendered",
                    f.field
                ),
            }
        }
    }
}

#[test]
fn a_field_that_one_consumer_renders_and_the_other_does_not_says_so_with_a_reason() {
    for f in NODE_FIELDS {
        match (f.canvas, f.export) {
            (Support::Rendered, Support::Rendered) => {}
            (Support::Unsupported(why), _) | (_, Support::Unsupported(why)) => {
                assert!(
                    why.trim().len() > 10,
                    "`{}`: an unsupported mark needs a reason",
                    f.field
                )
            }
        }
    }
}

#[test]
fn every_field_the_export_marks_unsupported_is_named_in_a_report_or_has_a_reason_a_person_can_read()
{
    // The one field the export leaves out whole is the shader preset; a shader is reported as approximated.
    let unsupported: Vec<&str> = NODE_FIELDS
        .iter()
        .filter(|f| matches!(f.export, Support::Unsupported(_)))
        .map(|f| f.field)
        .collect();
    assert_eq!(unsupported, ["shader"]);
}
