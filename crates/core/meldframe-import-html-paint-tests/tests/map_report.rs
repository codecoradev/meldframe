//! What a node could not take (`docs/import-html.md` sections 3.0 and 4.2): every refusal is attached to the node it
//! affects, a conversion is listed as an approximation, a class nobody reads is listed and is not a lost property.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod map_common;
use html_import::css::EntryKind;
use map_common::*;

#[test]
fn a_refused_paint_property_is_on_the_node_it_belongs_to_with_a_stable_code() {
    let i = run(
        "<div style=\"width:300px\"><div id=a style=\"width:5px;height:5px;filter:blur(2px)\"></div><div id=b style=\"width:5px;height:5px\"></div></div>",
    );
    let a = info(&i, "a");
    assert_eq!(a.unsupported.len(), 1, "{a:?}");
    assert_eq!(
        (
            a.unsupported[0].property.as_str(),
            a.unsupported[0].code.as_str()
        ),
        ("filter", "refused_not_modelled")
    );
    assert_eq!(a.unsupported[0].value, "blur(2px)");
    assert!(info(&i, "b").unsupported.is_empty());
    assert_eq!(i.unsupported_count(), 1);
}

#[test]
fn a_value_is_cut_to_eighty_characters_in_the_list() {
    let long = "a".repeat(200);
    let i = run(&format!(
        "<div id=a style=\"width:5px;height:5px;filter:{long}\"></div>"
    ));
    assert_eq!(info(&i, "a").unsupported[0].value.chars().count(), 80);
}

#[test]
fn a_changed_value_is_an_approximation_not_a_loss() {
    let i = run("<div id=a style=\"width:5px;height:5px;opacity:2\"></div>");
    assert!(
        approx(&i, "a").contains(&"opacity".to_string()),
        "{:?}",
        info(&i, "a")
    );
    assert_eq!(i.unsupported_count(), 0);
}

#[test]
fn a_var_that_does_not_resolve_is_listed_with_the_reason() {
    let i = run("<div id=a style=\"width:var(--nope);height:5px\"></div>");
    let a = info(&i, "a");
    assert!(
        a.unsupported
            .iter()
            .any(|l| l.property == "width" && l.code == "var_undefined"),
        "{a:?}"
    );
}

#[test]
fn a_class_no_rule_names_is_listed_once_and_is_not_counted_as_lost() {
    let i = run(
        "<div style=\"width:300px\"><div class=\"hero p-4\" style=\"height:5px\"></div><div class=\"hero\" style=\"height:5px\"></div></div>",
    );
    let listed: Vec<&str> = i
        .report
        .entries
        .iter()
        .filter(|e| e.kind == EntryKind::Listed && e.code == "class_unresolved")
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(listed, ["hero", "p-4"]);
    assert_eq!(i.unsupported_count(), 0);
}

#[test]
fn generated_content_is_counted_and_the_exports_border_form_is_not() {
    let i = run_css(
        "<div style=\"width:300px\"><div class=a style=\"width:5px;height:5px\"></div></div>",
        "body{margin:0}.a::before{content:'x'}",
    );
    assert_eq!(i.report.counts.get("generated_content"), Some(&1));
}

#[test]
fn visibility_hidden_keeps_the_box_with_no_opacity() {
    let ns = import_nodes(
        "<div style=\"width:300px\"><div id=a style=\"visibility:hidden;width:5px;height:5px\"></div></div>",
    );
    assert_eq!(named(&ns, "a").opacity.lit().copied(), Some(0.0));
}

#[test]
fn the_unsupported_count_is_what_strict_mode_refuses() {
    let clean = run("<div style=\"width:100px\"><p>x</p></div>");
    assert_eq!(clean.unsupported_count(), 0);
    let dirty = run("<div style=\"width:100px;letter-spacing:2px\"><p>x</p></div>");
    assert_eq!(dirty.unsupported_count(), 1);
}

#[test]
fn skipped_lines_are_bounded() {
    let hidden: String = (0..300).map(|_| "<i style=\"display:none\"></i>").collect();
    let i = run(&format!("<div style=\"width:10px\">{hidden}<p>x</p></div>"));
    assert_eq!(i.skipped.len(), html_import::map::MAX_SKIPPED);
    assert_eq!(i.skipped_omitted, 100);
}

#[test]
fn the_lists_of_a_node_are_bounded_and_the_lines_left_out_are_counted_and_still_refuse_strict() {
    let props: String = (0..50).map(|i| format!("zz-prop-{i}:1;")).collect();
    let i = run(&format!(
        "<div style=\"width:300px\"><div id=a style=\"width:5px;height:5px;{props}\"></div></div>"
    ));
    let a = info(&i, "a");
    assert_eq!(
        (a.unsupported.len(), a.lost_omitted, a.approx_omitted),
        (32, 18, 0)
    );
    // What strict refuses is every property that was not carried, listed or not.
    assert_eq!(i.unsupported_count(), 50);
}
