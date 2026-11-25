//! Auto layout: direction, gap, padding, margin, alignment, clipping and position (`docs/import-html.md` section 3.4).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::map::{Options, plan};
use meldframe_protocol::{Align, Bind, Justify, LayoutMode};

/// The node `name` of `<div data-name=Box style=...>` with two texts in it, laid out by `style`.
fn boxed(style: &str) -> Vec<meldframe_protocol::Node> {
    map(&format!(
        "<div data-name=Box style='{style}'><p>a</p><p>b</p></div>"
    ))
}

fn refused<'a>(p: &'a html_import::map::Plan, node: &str, prop: &str) -> Vec<&'a str> {
    p.nodes
        .iter()
        .find(|n| n.name == node)
        .unwrap()
        .unsupported
        .iter()
        .filter(|u| u.property == prop)
        .map(|u| u.code)
        .collect()
}

#[test]
fn flex_is_a_row_unless_the_direction_says_column() {
    for (style, want) in [
        ("display:flex", LayoutMode::Row),
        ("display:flex;flex-direction:row", LayoutMode::Row),
        (
            "display:inline-flex;flex-direction:column",
            LayoutMode::Column,
        ),
        ("display:flex;flex-direction:column", LayoutMode::Column),
    ] {
        assert_eq!(named(&boxed(style), "Box").layout, want, "{style}");
    }
}

#[test]
fn block_flow_is_a_column_with_no_gap_and_children_at_the_start() {
    let list = boxed("display:block");
    let b = named(&list, "Box");
    assert_eq!(
        (b.layout, b.gap, b.align, b.justify),
        (LayoutMode::Column, 0.0, Align::Start, Justify::Start)
    );
}

#[test]
fn a_reversed_direction_is_in_order_and_refused() {
    let p = planned(
        "<div data-name=Box style='display:flex;flex-direction:column-reverse'><p>a</p><p>b</p></div>",
        RESET,
    );
    assert_eq!(nodes(&p)[0].layout, LayoutMode::Column);
    assert_eq!(refused(&p, "Box", "flex-direction"), ["refused_reverse"]);
}

#[test]
fn grid_is_a_column_of_its_children_and_refused() {
    let p = planned(
        "<div data-name=Box style='display:grid'><p>a</p><p>b</p></div>",
        RESET,
    );
    assert_eq!(nodes(&p)[0].layout, LayoutMode::Column);
    assert_eq!(refused(&p, "Box", "display"), ["refused_grid"]);
}

#[test]
fn the_gap_of_the_main_axis_is_the_gap() {
    let gap = |style: &str| named(&boxed(style), "Box").gap;
    assert_eq!(gap("display:flex;gap:12px"), 12.0);
    assert_eq!(gap("display:flex;column-gap:7px"), 7.0);
    assert_eq!(
        gap("display:flex;flex-direction:column;row-gap:5px;column-gap:9px"),
        5.0
    );
    assert_eq!(
        gap("display:flex;gap:2px 9px"),
        9.0,
        "a row's gap is the column gap"
    );
    assert_eq!(
        gap("display:flex;row-gap:4px"),
        0.0,
        "only the cross gap is set"
    );
    let p = planned(
        "<div data-name=Box style='display:flex;gap:2px 9px'><p>a</p><p>b</p></div>",
        RESET,
    );
    assert_eq!(p.nodes[0].approximated[0].property, "gap");
    let p = planned(
        "<div data-name=Box style='display:flex;row-gap:4px'><p>a</p><p>b</p></div>",
        RESET,
    );
    assert_eq!(refused(&p, "Box", "row-gap"), ["refused_not_modelled"]);
}

#[test]
fn padding_is_per_side_in_pixels_and_em_and_rem_are_converted() {
    let list = boxed("padding:8px 4px 2px 6px");
    let pad = named(&list, "Box").pad();
    assert_eq!(
        (pad.top, pad.right, pad.bottom, pad.left),
        (8.0, 4.0, 2.0, 6.0)
    );
    let list = boxed("padding:5px");
    let b = named(&list, "Box");
    assert_eq!((b.padding, b.pad_t), (5.0, None));
    assert_eq!(named(&boxed("padding:1.5rem 2em"), "Box").pad().left, 32.0);
    assert_eq!(
        named(&boxed("display:flex"), "Box").pad().top,
        0.0,
        "the model's 16 is not CSS's 0"
    );
    let p = planned(
        "<div data-name=Box style='padding:2%'><p>a</p><p>b</p></div>",
        RESET,
    );
    assert_eq!(refused(&p, "Box", "padding-top"), ["refused_value"]);
    assert_eq!(nodes(&p)[0].pad().top, 0.0);
}

#[test]
fn margins_of_a_child_in_a_layout_are_kept_and_negative_or_auto_ones_are_refused() {
    let list = map(
        "<div data-name=Box style='display:flex'><p data-name=P style='margin:1px 2px 3px 4px'>a</p></div>",
    );
    let m = named(&list, "P").margin();
    assert_eq!((m.top, m.right, m.bottom, m.left), (1.0, 2.0, 3.0, 4.0));
    let p = planned(
        "<div data-name=Box style='display:flex'><p data-name=P style='margin:-4px auto 0 0'>a</p></div>",
        RESET,
    );
    assert_eq!(refused(&p, "P", "margin-top"), ["refused_value"]);
    assert_eq!(refused(&p, "P", "margin-right"), ["refused_value"]);
    assert_eq!(nodes(&p)[1].margin().top, 0.0);
}

#[test]
fn align_and_justify_read_the_values_the_model_has() {
    let ab = |style: &str| {
        let list = boxed(style);
        let b = named(&list, "Box");
        (b.align, b.justify)
    };
    assert_eq!(
        ab("display:flex;align-items:center;justify-content:space-between"),
        (Align::Center, Justify::Between)
    );
    assert_eq!(
        ab("display:flex;align-items:flex-end;justify-content:safe center"),
        (Align::End, Justify::Center)
    );
    assert_eq!(
        ab("display:flex;align-items:safe flex-end;justify-content:flex-end"),
        (Align::End, Justify::End)
    );
    assert_eq!(
        ab("display:flex;align-items:start;justify-content:normal"),
        (Align::Start, Justify::Start)
    );
    let p = planned(
        "<div data-name=Box style='display:flex;align-items:baseline;justify-content:space-evenly'><p>a</p></div>",
        RESET,
    );
    assert_eq!(refused(&p, "Box", "align-items"), ["refused_value"]);
    assert_eq!(refused(&p, "Box", "justify-content"), ["refused_value"]);
    assert_eq!(
        (nodes(&p)[0].align, nodes(&p)[0].justify),
        (Align::Start, Justify::Start)
    );
}

#[test]
fn overflow_clips_only_when_it_says_so() {
    let clip = |style: &str| named(&boxed(style), "Box").clip;
    assert!(clip("overflow:hidden"));
    assert!(clip("overflow:clip"));
    assert!(clip("overflow:auto"));
    assert!(clip("overflow-x:hidden"));
    assert!(!clip("overflow:visible"));
    assert!(!clip(""), "the model's default is true, CSS's is visible");
    let p = planned(
        "<div data-name=Box style='overflow:scroll'><p>a</p><p>b</p></div>",
        RESET,
    );
    assert_eq!(p.nodes[0].approximated[0].property, "overflow");
    assert!(
        !named(
            &map("<div data-name=Box><p data-name=P>a</p><p>b</p></div>"),
            "P"
        )
        .clip
    );
}

#[test]
fn absolute_children_sit_at_their_own_place_and_a_frame_of_only_those_has_no_layout() {
    let list = map(
        "<div data-name=Box style='position:relative;width:100px;height:100px'>\
         <p data-name=A style='position:absolute;left:10px;top:20px'>a</p>\
         <div data-name=B style='position:absolute;left:5px;top:6px;width:10px;height:10px'></div></div>",
    );
    let (b, a, c) = (named(&list, "Box"), named(&list, "A"), named(&list, "B"));
    assert_eq!(b.layout, LayoutMode::None);
    assert_eq!((a.x, a.y, a.absolute), (10.0, 20.0, false));
    assert_eq!((c.x, c.y), (5.0, 6.0));
    // In a layout the child leaves the flow and says so.
    let list = map(
        "<div data-name=Box style='display:flex'><p data-name=A style='position:absolute;left:10px;top:20px'>a</p><p>b</p></div>",
    );
    let a = named(&list, "A");
    assert_eq!((a.x, a.y, a.absolute), (10.0, 20.0, true));
    assert_eq!(named(&list, "Box").layout, LayoutMode::Row);
    // A block with a box in the flow besides is a column, and its absolute child leaves the flow.
    let list = map(
        "<div data-name=Box><p data-name=A style='position:absolute;left:1px;top:2px'>a</p><div data-name=B><p>b</p><p>c</p></div></div>",
    );
    assert_eq!(named(&list, "Box").layout, LayoutMode::Column);
    assert!(named(&list, "A").absolute);
}

#[test]
fn offsets_the_model_cannot_place_are_refused() {
    let p = planned(
        "<div data-name=Box style='display:flex'><p data-name=A style='position:absolute;right:4px;bottom:2px'>a</p>\
         <p data-name=B style='position:sticky;top:0'>b</p><p data-name=C style='position:relative;left:3px'>c</p></div>",
        RESET,
    );
    let list = nodes(&p);
    assert_eq!((named(&list, "A").x, named(&list, "A").y), (0.0, 0.0));
    assert_eq!(refused(&p, "A", "right"), ["refused_value"]);
    assert_eq!(refused(&p, "A", "bottom"), ["refused_value"]);
    assert_eq!(refused(&p, "B", "position"), ["refused_value"]);
    assert_eq!(refused(&p, "C", "left"), ["refused_value"]);
    assert!(!named(&list, "B").absolute && named(&list, "C").x == 0.0);
}

#[test]
fn visibility_hidden_keeps_the_box_with_no_opacity() {
    let list =
        map("<div data-name=Box><p data-name=P style='visibility:hidden'>a</p><p>b</p></div>");
    assert_eq!(named(&list, "P").opacity, Bind::Lit(0.0));
    assert_eq!(list[2].opacity, Bind::Lit(1.0));
}

#[test]
fn a_page_with_the_browsers_margin_gets_a_root_with_that_padding() {
    let p = plan(
        "<div data-name=Card><p>a</p><p>b</p></div>",
        "",
        &Options::default(),
    )
    .unwrap();
    let list = nodes(&p);
    assert_eq!(
        (list[0].name.as_str(), list[1].name.as_str()),
        ("Imported HTML", "Card")
    );
    assert_eq!(list[0].pad().top, 8.0);
    assert_eq!(list[0].layout, LayoutMode::Column);
    assert_eq!(p.nodes[0].approximated[0].property, "margin");
}

#[test]
fn the_padding_and_margin_of_the_body_and_html_are_the_roots_padding() {
    let html = "<html style='padding:3px'><body style='margin:2px;padding:5px'><div data-name=Card><p>a</p><p>b</p></div></body></html>";
    let list = nodes(&plan(html, "", &Options::default()).unwrap());
    assert_eq!(list[0].pad().left, 10.0);
    assert_eq!(list[1].name, "Card");
}
