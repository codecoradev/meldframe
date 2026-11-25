//! What is not read is listed, on the node it belongs to and in the report, and never dropped or guessed
//! (`docs/import-html.md` sections 3.2 to 3.6 and 4.2).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::html::EntryKind;
use html_import::map::Plan;

/// The properties the node `name` lists as not read, with their codes.
fn lost<'a>(p: &'a Plan, name: &str) -> Vec<(&'a str, &'a str)> {
    p.nodes
        .iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("no node {name}"))
        .unsupported
        .iter()
        .map(|u| (u.property.as_str(), u.code))
        .collect()
}

fn boxed(style: &str) -> Plan {
    planned(
        &format!(
            "<div data-name=Box style='{style}'><div data-name=A><p>a</p><p>b</p></div><p>c</p></div>"
        ),
        RESET,
    )
}

#[test]
fn a_property_nobody_reads_is_listed_on_the_node_and_in_the_report() {
    let p = boxed("transform:rotate(3deg);z-index:2;float:left");
    let l = lost(&p, "Box");
    assert_eq!(
        l,
        [
            ("float", "refused_not_modelled"),
            ("transform", "refused_not_modelled"),
            ("z-index", "refused_not_modelled")
        ]
    );
    let entries: Vec<_> = p
        .report
        .entries
        .iter()
        .filter(|e| e.kind == EntryKind::Refused)
        .collect();
    assert_eq!(entries.len(), 3, "{entries:?}");
    assert_eq!(
        (entries[0].name.as_str(), entries[0].code),
        ("float", "refused_not_modelled")
    );
    assert_eq!(count(&p, "refused_not_modelled"), 3);
    assert_eq!(entries[0].at, p.nodes[0].at);
}

#[test]
fn a_value_is_cut_to_eighty_characters() {
    let long = "x".repeat(300);
    let p = boxed(&format!("zoom:{long}"));
    let u = &p.nodes[0].unsupported[0];
    assert_eq!(u.value.chars().count(), 80);
}

#[test]
fn grid_properties_are_listed_one_by_one() {
    let p = boxed("display:grid;grid-template-columns:1fr 1fr;grid-gap:4px;grid-area:a");
    assert_eq!(
        lost(&p, "Box"),
        [
            ("display", "refused_grid"),
            ("grid-area", "refused_grid"),
            ("grid-gap", "refused_grid"),
            ("grid-template-columns", "refused_grid")
        ]
    );
}

#[test]
fn what_the_mapping_reads_and_what_paint_reads_is_not_listed_here() {
    let p = boxed(
        "display:flex;flex-direction:column;gap:4px;padding:2px;margin:1px;align-items:center;justify-content:center;\
         width:50px;height:40px;position:relative;overflow:hidden;color:#f00;font-size:12px;font-weight:700;\
         text-align:center;white-space:pre;text-transform:none;box-sizing:border-box;visibility:visible;\
         background:#fff;border:1px solid red;border-radius:4px;box-shadow:0 0 2px #000;opacity:.5;cursor:pointer;filter:blur(2px)",
    );
    assert!(lost(&p, "Box").is_empty(), "{:?}", lost(&p, "Box"));
}

#[test]
fn the_export_s_own_forms_are_not_a_loss() {
    let p = boxed(
        "flex-shrink:0;flex-shrink:1;min-width:0;min-height:0;flex-wrap:nowrap;max-width:none;z-index:auto;transform:none",
    );
    assert!(lost(&p, "Box").is_empty(), "{:?}", lost(&p, "Box"));
}

#[test]
fn flex_wrap_and_a_shrink_the_model_does_not_have_are_refused() {
    let p = boxed("display:flex;flex-wrap:wrap;flex-shrink:3");
    assert_eq!(
        lost(&p, "Box"),
        [
            ("flex-shrink", "refused_value"),
            ("flex-wrap", "refused_not_modelled")
        ]
    );
}

#[test]
fn the_refusals_of_the_css_stage_are_listed_on_the_element_with_their_codes() {
    let p = boxed("width:var(--missing);font-size:1vw");
    let l = lost(&p, "Box");
    assert!(l.contains(&("width", "var_undefined")), "{l:?}");
    assert!(l.contains(&("font-size", "font_size_unreadable")), "{l:?}");
}

#[test]
fn a_font_the_model_has_no_field_for_is_listed_on_every_text_it_reaches() {
    let p = planned(
        "<div data-name=Box style='font-family:Georgia;line-height:1.5;letter-spacing:2px;font-style:normal'>bare<p data-name=P>a</p></div>",
        RESET,
    );
    let expect = [
        ("font-family", "refused_not_modelled"),
        ("letter-spacing", "refused_not_modelled"),
        ("line-height", "refused_not_modelled"),
    ];
    assert_eq!(lost(&p, "P"), expect);
    assert_eq!(lost(&p, "Box text"), expect, "an anonymous text too");
    assert!(
        lost(&p, "Box").is_empty(),
        "the frame has no font: {:?}",
        lost(&p, "Box")
    );
}

#[test]
fn what_an_inline_element_that_is_joined_loses_is_listed_on_the_text() {
    let p = planned(
        "<div data-name=Box><p data-name=T>a <a href=x style='text-decoration:underline;color:#00f'>b</a></p><p>y</p></div>",
        RESET,
    );
    assert_eq!(lost(&p, "T"), [("text-decoration", "refused_not_modelled")]);
    assert_eq!(
        p.nodes.iter().find(|n| n.name == "T").unwrap().approximated[0].property,
        "color"
    );
}

#[test]
fn a_plain_wrapper_that_goes_hands_its_losses_to_the_node_that_takes_its_place() {
    let p = planned(
        "<div data-name=Top><div style='zoom:2'><div data-name=In><p>a</p><p>b</p></div></div></div>",
        RESET,
    );
    assert_eq!(lost(&p, "In"), [("zoom", "refused_not_modelled")]);
}

#[test]
fn colspan_and_rowspan_are_refused() {
    let p = planned(
        "<table data-name=T><tr data-name=R><td data-name=A colspan=2 rowspan=3>a</td><td data-name=B colspan=1>b</td></tr></table>",
        RESET,
    );
    assert_eq!(
        lost(&p, "A"),
        [
            ("colspan", "refused_not_modelled"),
            ("rowspan", "refused_not_modelled")
        ]
    );
    assert!(lost(&p, "B").is_empty());
}

#[test]
fn attributes_the_model_has_no_place_for_are_counted() {
    let p = planned(
        "<div data-name=Box><a href=x title=t aria-label=l role=button><span>go</span></a><p id=i class=c data-node=n>a</p></div>",
        RESET,
    );
    assert_eq!(count(&p, "attribute_ignored"), 4);
}

#[test]
fn generated_content_has_no_node_and_is_refused() {
    let p = planned(
        "<div data-name=Box class=x><p>a</p><p>b</p></div>",
        &format!("{RESET}.x::before{{content:\"a\"}}.x::after{{content:\"b\";color:red}}"),
    );
    let codes: Vec<_> = p.nodes[0]
        .unsupported
        .iter()
        .map(|u| (u.property.as_str(), u.code))
        .collect();
    assert_eq!(
        codes,
        [
            ("::before", "refused_generated_content"),
            ("::after", "refused_generated_content")
        ]
    );
}

#[test]
fn the_after_box_that_carries_a_border_is_the_exports_and_not_generated_content() {
    let p = planned(
        "<div data-name=Box class=x><p>a</p><p>b</p></div>",
        &format!(
            "{RESET}.x::after{{content:\"\";position:absolute;pointer-events:none;inset:0;border:1px solid #000}}"
        ),
    );
    assert!(lost(&p, "Box").is_empty(), "{:?}", lost(&p, "Box"));
}

#[test]
fn an_anonymous_text_does_not_list_what_its_frame_declares_again() {
    let p = planned(
        "<div data-name=Box style='float:left'>bare<div><p>a</p><p>b</p></div></div>",
        RESET,
    );
    assert_eq!(lost(&p, "Box"), [("float", "refused_not_modelled")]);
    assert!(lost(&p, "Box text").is_empty());
}
