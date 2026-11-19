//! E3.3: the render step resolves bound fields through the variables. The mode a viewer picks changes what is
//! painted and never the nodes; a binding that cannot be resolved paints the default of the field.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_protocol::VarType;
use meldframe_protocol::{Doc, Node};
use meldframe_render::coverage::NODE_FIELDS;
use meldframe_render::{ModeChoice, PaintedBox, resolve, resolve_with};
use meldframe_variables::node_bindings;
use serde_json::{Value, json};

fn nodes(v: Value) -> Vec<Node> {
    serde_json::from_value(v).unwrap()
}

/// `appearance` (light, dark): `bg` and `brand` (an alias of `base`'s blue), the numbers `round` and `fade`.
fn doc(bound: Value) -> Doc {
    let mut d = Doc::of_nodes(nodes(bound));
    for c in [
        json!({"type": "addCollection", "collection": {"id": "base", "name": "base", "modes": [{"id": "m", "name": "m"}]}}),
        json!({"type": "addCollection", "collection": {"id": "appearance", "name": "appearance",
            "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}}),
        json!({"type": "addVariable", "variable": {"id": "blue", "collection": "base", "name": "color/blue", "type": "color",
            "values": {"m": "#2563eb"}}}),
        json!({"type": "addVariable", "variable": {"id": "bg", "collection": "appearance", "name": "color/bg", "type": "color",
            "values": {"light": "#ffffff", "dark": "#101010"}}}),
        json!({"type": "addVariable", "variable": {"id": "brand", "collection": "appearance", "name": "color/brand", "type": "color",
            "values": {"light": {"var": "blue"}, "dark": "#38bdf8"}}}),
        json!({"type": "addVariable", "variable": {"id": "round", "collection": "appearance", "name": "radius/card", "type": "number",
            "values": {"light": 8, "dark": 16}}}),
        json!({"type": "addVariable", "variable": {"id": "fade", "collection": "appearance", "name": "opacity/muted", "type": "number",
            "values": {"light": 0.9, "dark": 0.5}}}),
    ] {
        meldframe_core::apply_doc(
            &mut d,
            &serde_json::from_value(c).unwrap(),
            &Default::default(),
        )
        .unwrap();
    }
    d
}

fn bound() -> Doc {
    doc(json!([
        {"id": "a", "type": "rect", "pos": "a0", "fill": {"var": "bg"}, "radius": {"var": "round"}, "opacity": {"var": "fade"}},
        {"id": "b", "type": "rect", "pos": "a1", "fill": {"var": "brand"}}
    ]))
}

fn painted(d: &Doc, choice: &ModeChoice) -> Vec<PaintedBox> {
    resolve_with(&d.nodes, &d.vars, choice)
}

#[test]
fn a_bound_field_paints_the_value_of_its_variable() {
    let boxes = painted(&bound(), &ModeChoice::default());
    assert_eq!(boxes[0].fill, Some(0xffffff));
    assert_eq!((boxes[0].radius, boxes[0].opacity), (8.0, 0.9));
    assert_eq!(
        boxes[1].fill,
        Some(0x2563eb),
        "an alias is followed to its target"
    );
}

#[test]
fn switching_the_mode_repaints_and_never_changes_the_nodes() {
    let d = bound();
    let before = serde_json::to_string(&d.nodes).unwrap();
    let dark = ModeChoice::of([("appearance", "dark")]);
    let boxes = painted(&d, &dark);
    assert_eq!(boxes[0].fill, Some(0x101010));
    assert_eq!((boxes[0].radius, boxes[0].opacity), (16.0, 0.5));
    assert_eq!(boxes[1].fill, Some(0x38bdf8));
    assert_eq!(serde_json::to_string(&d.nodes).unwrap(), before);
    // Back to the default mode: the first paint again.
    assert_eq!(
        painted(&d, &ModeChoice::default()),
        painted(&d, &ModeChoice::of([("appearance", "light")]))
    );
}

#[test]
fn a_binding_that_cannot_be_resolved_paints_the_default_of_the_field() {
    // The variable is not in the file (a bad file that the load check would refuse): nothing panics.
    let boxes = resolve(&nodes(json!([
        {"id": "a", "type": "rect", "fill": {"var": "gone"}, "radius": {"var": "gone"}, "opacity": {"var": "gone"}}
    ])));
    assert_eq!(
        (boxes[0].fill, boxes[0].radius, boxes[0].opacity),
        (Some(0x3f3f46), 0.0, 1.0)
    );
}

#[test]
fn a_resolved_number_outside_the_range_of_its_field_is_held_inside_it() {
    let mut d = bound();
    d.vars.variables[3].values.insert(
        "light".into(),
        meldframe_protocol::Bind::Lit(meldframe_protocol::Value::Number(-4.0)),
    );
    d.vars.variables[4].values.insert(
        "light".into(),
        meldframe_protocol::Bind::Lit(meldframe_protocol::Value::Number(7.0)),
    );
    let b = &painted(&d, &ModeChoice::default())[0];
    assert_eq!((b.radius, b.opacity), (0.0, 1.0));
}

#[test]
fn the_coverage_table_names_exactly_the_fields_that_can_be_bound_and_each_paints_its_value() {
    // The fields the table marks as bindable are the ones the variables crate reports for a node that binds
    // them all: one list in the model, one in the table, and a test that they agree.
    let all: Node = serde_json::from_value(json!({"id": "n", "type": "rect",
        "fill": {"var": "x"}, "radius": {"var": "x"}, "opacity": {"var": "x"}}))
    .unwrap();
    let mut in_model: Vec<&str> = node_bindings(&all).iter().map(|b| b.field).collect();
    let mut in_table: Vec<&str> = NODE_FIELDS
        .iter()
        .filter(|f| f.bindable.is_some())
        .map(|f| f.field)
        .collect();
    in_model.sort_unstable();
    in_table.sort_unstable();
    assert_eq!(in_table, in_model);
    // ... and they agree on the type of the variable each field takes.
    let mut types_in_model: Vec<(&str, VarType)> = node_bindings(&all)
        .iter()
        .map(|b| (b.field, b.expects))
        .collect();
    let mut types_in_table: Vec<(&str, VarType)> = NODE_FIELDS
        .iter()
        .filter_map(|f| f.bindable.map(|t| (f.field, t)))
        .collect();
    types_in_model.sort_unstable_by_key(|t| t.0);
    types_in_table.sort_unstable_by_key(|t| t.0);
    assert_eq!(types_in_table, types_in_model);
    // A bound field paints exactly what the literal of its resolved value paints, in every mode.
    for (mode, literal) in [
        (
            "light",
            json!({"fill": "#ffffff", "radius": 8, "opacity": 0.9}),
        ),
        (
            "dark",
            json!({"fill": "#101010", "radius": 16, "opacity": 0.5}),
        ),
    ] {
        let d = doc(
            json!([{"id": "a", "type": "rect", "fill": {"var": "bg"}, "radius": {"var": "round"}, "opacity": {"var": "fade"}}]),
        );
        let mut want = json!({"id": "a", "type": "rect"});
        want.as_object_mut()
            .unwrap()
            .extend(literal.as_object().unwrap().clone());
        let lit = resolve(&nodes(json!([want])));
        assert_eq!(
            painted(&d, &ModeChoice::of([("appearance", mode)])),
            lit,
            "{mode}"
        );
    }
}

fn set(d: &mut Doc, c: Value) {
    meldframe_core::apply_doc(d, &serde_json::from_value(c).unwrap(), &Default::default()).unwrap();
}

#[test]
fn a_cycle_in_a_bad_file_paints_the_default_and_does_not_hang() {
    // The load check refuses a cycle; the render step must still survive one.
    let mut d = bound();
    let brand = d
        .vars
        .variables
        .iter_mut()
        .find(|v| v.id == "brand")
        .unwrap();
    brand
        .values
        .insert("light".into(), meldframe_protocol::Bind::var("bg"));
    let bg = d.vars.variables.iter_mut().find(|v| v.id == "bg").unwrap();
    bg.values
        .insert("light".into(), meldframe_protocol::Bind::var("brand"));
    let boxes = painted(&d, &ModeChoice::default());
    assert_eq!(
        (boxes[0].fill, boxes[1].fill),
        (Some(0x3f3f46), Some(0x3f3f46))
    );
}

#[test]
fn a_variable_of_the_wrong_type_paints_the_default_of_the_field() {
    // A colour variable on a number field and a number variable on the fill (a bad file).
    let d = doc(json!([
        {"id": "a", "type": "rect", "fill": {"var": "round"}, "radius": {"var": "bg"}, "opacity": {"var": "bg"}}
    ]));
    let b = &painted(&d, &ModeChoice::default())[0];
    assert_eq!((b.fill, b.radius, b.opacity), (Some(0x3f3f46), 0.0, 1.0));
}

#[test]
fn a_chosen_mode_that_the_collection_does_not_have_shows_the_default_mode() {
    let d = bound();
    assert_eq!(
        painted(&d, &ModeChoice::of([("appearance", "no-such-mode")])),
        painted(&d, &ModeChoice::default())
    );
}

#[test]
fn two_collections_are_picked_independently_through_an_alias() {
    // `accent` (appearance: light, dark) aliases `swatch` of `palette` (warm, cool); each collection takes its
    // own pick, and a collection that is not picked shows its default.
    let mut d = doc(json!([{"id": "a", "type": "rect", "fill": {"var": "accent"}}]));
    for c in [
        json!({"type": "addCollection", "collection": {"id": "palette", "name": "palette",
            "modes": [{"id": "warm", "name": "warm"}, {"id": "cool", "name": "cool"}]}}),
        json!({"type": "addVariable", "variable": {"id": "swatch", "collection": "palette", "name": "color/swatch", "type": "color",
            "values": {"warm": "#ff8800", "cool": "#0088ff"}}}),
        json!({"type": "addVariable", "variable": {"id": "accent", "collection": "appearance", "name": "color/accent", "type": "color",
            "values": {"light": {"var": "swatch"}, "dark": "#222222"}}}),
    ] {
        set(&mut d, c);
    }
    let fill =
        |choice: &[(&str, &str)]| painted(&d, &ModeChoice::of(choice.iter().copied()))[0].fill;
    assert_eq!(fill(&[]), Some(0xff8800));
    assert_eq!(fill(&[("palette", "cool")]), Some(0x0088ff));
    assert_eq!(
        fill(&[("appearance", "light"), ("palette", "cool")]),
        Some(0x0088ff)
    );
    assert_eq!(
        fill(&[("appearance", "dark"), ("palette", "cool")]),
        Some(0x222222),
        "dark does not alias the palette"
    );
    assert_eq!(fill(&[("appearance", "dark")]), Some(0x222222));
}
