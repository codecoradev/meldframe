//! E4.1: collections, modes and bound fields as CSS variables (ADR-0019).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod support;
use export::Format;
use meldframe_protocol::Variables;
use meldframe_render::ModeChoice;
use serde_json::json;
use support::*;

fn file() -> Variables {
    vars(json!({
        "collections": [
            {"id": "c1", "name": "appearance", "modes": [{"id": "m1", "name": "Light"}, {"id": "m2", "name": "Dark"}]},
            {"id": "c2", "name": "sizes", "modes": [{"id": "m3", "name": "Default"}]}
        ],
        "variables": [
            {"id": "v1", "collection": "c1", "name": "color/brand", "type": "color", "values": {"m1": "#112233", "m2": "#aabbcc"}},
            {"id": "v2", "collection": "c2", "name": "radius/card", "type": "number", "values": {"m3": 12}},
            {"id": "v3", "collection": "c2", "name": "opacity/muted", "type": "number", "values": {"m3": 0.4}},
            {"id": "v4", "collection": "c1", "name": "color/accent", "type": "color", "values": {"m1": {"var": "v1"}, "m2": "#000000"}},
            {"id": "v5", "collection": "c1", "name": "color/unused", "type": "color", "values": {"m1": "#ffffff", "m2": "#ffffff"}}
        ]
    }))
}

fn bound() -> Vec<meldframe_protocol::Node> {
    nodes(frame(
        json!({"fill": {"var": "v4"}}),
        vec![
            json!({"id": "r", "type": "rect", "fill": {"var": "v1"}, "radius": {"var": "v2"}, "opacity": {"var": "v3"}}),
        ],
    ))
}

fn go(modes: ModeChoice, format: Format) -> export::Export {
    run(&bound(), &file(), format, modes)
}

#[test]
fn a_bound_field_is_a_var_reference_and_the_unit_is_added_where_it_is_a_length() {
    let e = go(ModeChoice::default(), Format::Css);
    let r = rule(&e, "r").unwrap();
    assert!(
        r.contains("background-color:var(--appearance-color-brand)"),
        "{r}"
    );
    assert!(
        r.contains("border-radius:calc(var(--sizes-radius-card) * 1px)"),
        "{r}"
    );
    assert!(r.contains("opacity:var(--sizes-opacity-muted)"), "{r}");
    assert!(
        rule(&e, "f")
            .unwrap()
            .contains("background-color:var(--appearance-color-accent)")
    );
}

#[test]
fn the_default_mode_is_root_and_each_other_mode_is_a_rule_on_root() {
    let e = go(ModeChoice::default(), Format::Css);
    assert!(e.css.contains(":root{--appearance-color-brand:#112233;--appearance-color-accent:var(--appearance-color-brand);--sizes-radius-card:12;--sizes-opacity-muted:0.4}"), "{}", e.css);
    assert!(e.css.contains(":root[data-mode-appearance=\"dark\"]{--appearance-color-brand:#aabbcc;--appearance-color-accent:#000000}"), "{}", e.css);
    assert!(
        !e.css.contains("unused"),
        "a variable the frame does not use stays out: {}",
        e.css
    );
    assert!(
        !e.css.contains("data-mode-sizes"),
        "a collection with one mode has no mode rule"
    );
    assert!(
        e.html.contains("<html lang=\"en\">"),
        "the default mode is no attribute: {}",
        e.html
    );
}

#[test]
fn the_mode_picked_is_an_attribute_of_html_and_it_is_the_same_css() {
    let light = go(ModeChoice::default(), Format::Css);
    let dark = go(ModeChoice::of([("c1", "m2")]), Format::Css);
    assert!(
        dark.html
            .contains("<html lang=\"en\" data-mode-appearance=\"dark\">"),
        "{}",
        dark.html
    );
    assert_eq!(
        light.css, dark.css,
        "switching the mode changes no rule, only the attribute"
    );
}

#[test]
fn an_alias_is_a_var_of_the_other_variable_and_reaches_it_in_every_mode() {
    let e = go(ModeChoice::of([("c1", "m2")]), Format::Css);
    // `accent` aliases `brand` in the light mode only; the export keeps the alias, the browser follows the mode.
    assert!(
        e.css
            .contains("--appearance-color-accent:var(--appearance-color-brand)")
    );
}

#[test]
fn an_alias_pulls_in_the_variable_it_points_at() {
    let n = nodes(frame(json!({"fill": {"var": "v4"}}), vec![]));
    let e = run(&n, &file(), Format::Css, ModeChoice::default());
    assert!(
        e.css.contains("--appearance-color-brand:#112233"),
        "{}",
        e.css
    );
}

#[test]
fn the_tailwind_form_reads_the_same_variables() {
    let e = go(ModeChoice::default(), Format::Tailwind);
    let c = classes(&e, "r");
    for want in [
        "bg-[color:var(--appearance-color-brand)]",
        "rounded-[calc(var(--sizes-radius-card)_*_1px)]",
        "opacity-[var(--sizes-opacity-muted)]",
    ] {
        assert!(c.contains(&want), "{want} is not in {c:?}");
    }
    assert!(
        e.css.contains(":root{--appearance-color-brand:#112233"),
        "{}",
        e.css
    );
    assert!(
        e.css.contains(":root[data-mode-appearance=\"dark\"]"),
        "{}",
        e.css
    );
}

#[test]
fn a_colour_is_written_as_the_canvas_paints_it() {
    let v = vars(
        json!({"collections": [{"id": "c", "name": "t", "modes": [{"id": "m", "name": "a"}]}],
        "variables": [{"id": "v", "collection": "c", "name": "c/x", "type": "color", "values": {"m": "#aabbcc80"}}]}),
    );
    let n = nodes(frame(json!({"fill": {"var": "v"}}), vec![]));
    let e = run(&n, &v, Format::Css, ModeChoice::default());
    assert!(
        e.css.contains("--t-c-x:#aabbcc"),
        "the alpha is dropped: {}",
        e.css
    );
}

#[test]
fn a_binding_that_does_not_resolve_is_the_default_and_a_warning() {
    let n = nodes(frame(
        json!({"fill": {"var": "gone"}, "opacity": {"var": "gone"}, "radius": {"var": "gone"}}),
        vec![],
    ));
    let e = run(
        &n,
        &Variables::default(),
        Format::Css,
        ModeChoice::default(),
    );
    let f = rule(&e, "f").unwrap();
    assert!(f.contains("background-color:#3f3f46"), "{f}");
    assert!(!f.contains("opacity") && !f.contains("radius"), "{f}");
    assert_eq!(
        e.report
            .warnings
            .iter()
            .filter(|w| w.code == "unresolved_binding")
            .count(),
        3,
        "{:?}",
        e.report
    );
    assert!(!e.css.contains(":root"), "no variable, no rule");
}

#[test]
fn names_that_collide_get_a_suffix_and_a_file_without_variables_has_no_root_rule() {
    let v = vars(
        json!({"collections": [{"id": "c", "name": "a", "modes": [{"id": "m", "name": "x"}]}, {"id": "d", "name": "a/b", "modes": [{"id": "n", "name": "x"}]}],
        "variables": [
            {"id": "v1", "collection": "c", "name": "b-c", "type": "number", "values": {"m": 1}},
            {"id": "v2", "collection": "d", "name": "c", "type": "number", "values": {"n": 2}}]}),
    );
    let n = nodes(frame(
        json!({}),
        vec![
            json!({"id": "r", "type": "rect", "opacity": {"var": "v1"}}),
            json!({"id": "s", "type": "rect", "opacity": {"var": "v2"}}),
        ],
    ));
    let e = run(&n, &v, Format::Css, ModeChoice::default());
    assert!(e.css.contains("--a-b-c:1;--a-b-c-2:2"), "{}", e.css);
    let plain = css(frame(json!({}), vec![]));
    assert!(!plain.css.contains(":root"));
}

#[test]
fn the_export_is_the_same_for_every_call() {
    let (a, b) = (
        go(ModeChoice::of([("c1", "m2")]), Format::Tailwind),
        go(ModeChoice::of([("c1", "m2")]), Format::Tailwind),
    );
    assert_eq!((a.html.clone(), a.css.clone()), (b.html, b.css));
    assert_eq!(
        a.report,
        go(ModeChoice::of([("c1", "m2")]), Format::Tailwind).report
    );
    // The modes picked are the only thing that changes the document: another pick gives another export.
    let light = go(ModeChoice::default(), Format::Tailwind);
    assert_ne!(a.html, light.html);
    assert_eq!(
        a.css, light.css,
        "every mode is in the stylesheet either way"
    );
}

#[test]
fn a_variable_with_no_value_in_a_mode_is_a_warning_not_a_silent_var() {
    let v = vars(json!({
        "collections": [{"id": "c1", "name": "appearance", "modes": [{"id": "m1", "name": "Light"}, {"id": "m2", "name": "Dark"}]}],
        "variables": [{"id": "v1", "collection": "c1", "name": "brand", "type": "color", "values": {}}]
    }));
    let list = nodes(frame(json!({"fill": {"var": "v1"}}), vec![]));
    let e = run(&list, &v, Format::Css, ModeChoice::default());
    assert!(
        e.css.contains("background-color:var(--appearance-brand)"),
        "{}",
        e.css
    );
    assert!(
        e.report.warnings.iter().any(|w| w.code == "missing_value"),
        "{:?}",
        e.report
    );
}

#[test]
fn a_colour_variable_that_holds_no_colour_is_a_warning() {
    let v = vars(json!({
        "collections": [{"id": "c1", "name": "appearance", "modes": [{"id": "m1", "name": "Light"}]}],
        "variables": [{"id": "v1", "collection": "c1", "name": "brand", "type": "color", "values": {"m1": "red"}}]
    }));
    let list = nodes(frame(json!({"fill": {"var": "v1"}}), vec![]));
    let e = run(&list, &v, Format::Css, ModeChoice::default());
    assert!(
        e.css.contains("--appearance-brand:transparent"),
        "{}",
        e.css
    );
    assert!(e.report.warnings.iter().any(|w| w.code == "invalid_colour"));
}

#[test]
fn a_collection_or_mode_in_the_options_that_is_not_in_the_file_is_a_warning() {
    for modes in [
        ModeChoice::of([("nope", "m2")]),
        ModeChoice::of([("c1", "nope")]),
    ] {
        let e = go(modes, Format::Css);
        assert!(
            e.report.warnings.iter().any(|w| w.code == "unknown_mode"),
            "{:?}",
            e.report
        );
    }
    let e = go(ModeChoice::of([("c1", "m2")]), Format::Css);
    assert!(e.report.warnings.iter().all(|w| w.code != "unknown_mode"));
}
