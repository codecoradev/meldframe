//! Layout and sizing (`docs/import-html.md` section 3.4): flex and block flow, the size modes, padding, margin, the
//! model's defaults that CSS does not share, absolute boxes, and what is refused.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod map_common;
use map_common::*;
use meldframe_protocol::{Align, Justify, LayoutMode, NodeKind, SizeMode};

const FLEX: &str = "display:flex;width:400px";

fn one(style: &str, inner: &str) -> Vec<meldframe_protocol::Node> {
    import_nodes(&format!("<div id=a style=\"{style}\">{inner}</div>"))
}

#[test]
fn a_flex_row_has_its_gap_alignment_and_justification() {
    let ns = one(
        &format!("{FLEX};gap:12px;align-items:center;justify-content:space-between"),
        "<div id=b style=\"width:10px;height:10px\"></div><div id=c style=\"width:10px;height:10px\"></div>",
    );
    let a = &ns[0];
    assert_eq!(
        (a.layout, a.gap, a.align, a.justify),
        (LayoutMode::Row, 12.0, Align::Center, Justify::Between)
    );
    assert_eq!(a.sw, SizeMode::Fixed);
}

#[test]
fn a_flex_column_and_the_values_the_export_writes() {
    let ns = one(
        "display:flex;flex-direction:column;width:100px;height:80px;align-items:flex-end;justify-content:safe center",
        "<div id=b style=\"width:10px;height:10px\"></div>",
    );
    let a = &ns[0];
    assert_eq!(
        (a.layout, a.align, a.justify),
        (LayoutMode::Column, Align::End, Justify::Center)
    );
    assert_eq!((a.sh, a.h), (SizeMode::Fixed, 80.0));
    let b = named(&ns, "b");
    assert_eq!(
        (b.sw, b.sh, b.w, b.h),
        (SizeMode::Fixed, SizeMode::Fixed, 10.0, 10.0)
    );
}

#[test]
fn a_reverse_direction_wrap_and_other_justifications_are_refused_with_the_model_values_kept() {
    let i = run(
        "<div id=a style=\"display:flex;width:100px;flex-direction:row-reverse;flex-wrap:wrap;justify-content:space-around;align-items:baseline\"><div style=\"width:5px;height:5px\"></div></div>",
    );
    let a = &nodes(&i)[0];
    assert_eq!(
        (a.layout, a.justify, a.align),
        (LayoutMode::Row, Justify::Start, Align::Start)
    );
    let l = lost(&i, "a");
    for p in [
        "flex-direction",
        "flex-wrap",
        "justify-content",
        "align-items",
    ] {
        assert!(l.contains(&p.to_string()), "{p} in {l:?}");
    }
}

#[test]
fn a_block_is_a_column_with_no_gap_and_a_block_child_fills_the_width() {
    let ns = one(
        "width:300px",
        "<div id=b style=\"height:20px;background:#000000\"></div><p id=t style=\"margin:0\">hi</p>",
    );
    let a = &ns[0];
    assert_eq!(
        (a.layout, a.gap, a.align),
        (LayoutMode::Column, 0.0, Align::Start)
    );
    let b = named(&ns, "b");
    assert_eq!(modes(b), (SizeMode::Fill, SizeMode::Fixed));
    let t = named(&ns, "t");
    assert_eq!(
        modes(t),
        (SizeMode::Fill, SizeMode::Hug),
        "a text block is fill wide and hug high"
    );
}

#[test]
fn a_hug_parent_never_has_a_fill_child_on_that_axis() {
    // Rule G1: a fill under a hug parent would be exported as fixed, so the child hugs.
    let ns = one(
        "display:flex;align-items:flex-start",
        "<div id=b style=\"display:flex;height:10px\"><div id=c style=\"flex:1;height:5px\"></div></div>",
    );
    assert_eq!(
        ns[0].sw,
        SizeMode::Fixed,
        "the page is 1440 wide, the root is fixed"
    );
    let b = named(&ns, "b");
    assert_eq!(b.sw, SizeMode::Hug);
    let c = named(&ns, "c");
    assert_eq!(c.sw, SizeMode::Hug, "flex:1 under a row that hugs is hug");
}

#[test]
fn flex_one_is_fill_on_the_main_axis_and_the_export_flex_forms_too() {
    let ns = one(
        FLEX,
        "<div id=b style=\"flex:1 1 0%;min-width:0;height:5px\"></div><div id=c style=\"flex:1 1 40px;min-width:0;height:5px\"></div><div id=d style=\"width:30px;height:5px;flex-shrink:0\"></div>",
    );
    assert_eq!(named(&ns, "b").sw, SizeMode::Fill);
    assert_eq!(named(&ns, "c").sw, SizeMode::Fill);
    let d = named(&ns, "d");
    assert_eq!((d.sw, d.w), (SizeMode::Fixed, 30.0));
}

#[test]
fn the_cross_axis_stretches_only_when_the_parent_says_so() {
    let tall = format!("{FLEX};height:50px");
    let stretch = one(&tall, "<div id=b style=\"width:5px\"></div>");
    assert_eq!(named(&stretch, "b").sh, SizeMode::Fill);
    let start = one(
        &format!("{tall};align-items:flex-start"),
        "<div id=b style=\"width:5px\"></div>",
    );
    assert_eq!(named(&start, "b").sh, SizeMode::Hug);
    let own = one(
        &format!("{tall};align-items:flex-start"),
        "<div id=b style=\"width:5px;align-self:stretch\"></div>",
    );
    assert_eq!(named(&own, "b").sh, SizeMode::Fill);
}

#[test]
fn width_fit_content_is_hug_and_one_hundred_percent_is_fill() {
    let ns = one(
        "width:300px",
        "<div id=b style=\"width:fit-content;height:5px\"></div><div id=c style=\"width:100%;height:5px\"></div><div id=d style=\"width:50%;height:5px\"></div>",
    );
    assert_eq!(named(&ns, "b").sw, SizeMode::Hug);
    assert_eq!(named(&ns, "c").sw, SizeMode::Fill);
    assert_eq!(
        named(&ns, "d").sw,
        SizeMode::Fill,
        "another percentage is refused and the width stays auto"
    );
}

#[test]
fn padding_per_side_margin_and_the_zero_defaults() {
    let ns = one(
        "width:300px;padding:1px 2px 3px 4px",
        "<div id=b style=\"width:10px;height:10px;margin:5px 6px 7px 8px\"></div>",
    );
    let a = &ns[0];
    let p = a.pad();
    assert_eq!((p.top, p.right, p.bottom, p.left), (1.0, 2.0, 3.0, 4.0));
    let b = named(&ns, "b");
    assert_eq!((b.mar_t, b.mar_r, b.mar_b, b.mar_l), (5.0, 6.0, 7.0, 8.0));
    // One padding for all four sides is `padding`, with no side set.
    let u = one("width:100px;padding:6px", "<p>x</p>");
    assert_eq!((u[0].padding, u[0].pad_t, u[0].pad_l), (6.0, None, None));
    assert_eq!(
        u[1].mar_t, 16.0,
        "a paragraph keeps the browser margin of 1em"
    );
}

#[test]
fn a_negative_margin_and_margin_auto_are_dropped_and_listed() {
    let i = run(
        "<div id=a style=\"width:300px\"><div id=b style=\"width:10px;height:10px;margin:-4px auto\"></div></div>",
    );
    let b = named(&nodes(&i), "b").clone();
    assert_eq!((b.mar_t, b.mar_l), (0.0, 0.0));
    assert!(
        lost(&i, "b").contains(&"margin-top".to_string()),
        "{:?}",
        info(&i, "b")
    );
}

#[test]
fn the_model_size_is_the_border_box_so_content_box_sizes_grow() {
    let ns = one(
        "width:300px",
        "<div id=b style=\"width:100px;height:50px;padding:10px;border:2px solid #000000\"><p>x</p></div><div id=c style=\"box-sizing:border-box;width:100px;height:50px;padding:10px\"><p>x</p></div>",
    );
    let b = named(&ns, "b");
    assert_eq!((b.w, b.h), (124.0, 74.0));
    let c = named(&ns, "c");
    assert_eq!((c.w, c.h), (100.0, 50.0));
}

#[test]
fn an_absolute_box_has_its_place_and_a_frame_of_only_absolute_boxes_has_no_layout() {
    let ns = one(
        "position:relative;width:200px;height:100px",
        "<div id=b style=\"position:absolute;left:10px;top:20px;width:30px;height:40px\"></div>",
    );
    let a = &ns[0];
    assert_eq!(a.layout, LayoutMode::None);
    let b = named(&ns, "b");
    assert_eq!((b.x, b.y, b.absolute), (10.0, 20.0, false));
}

#[test]
fn an_absolute_box_in_a_flex_frame_leaves_the_flow() {
    let ns = one(
        &format!("{FLEX};position:relative"),
        "<div id=b style=\"width:10px;height:10px\"></div><div id=c style=\"position:absolute;left:7px;top:8px;width:5px;height:5px\"></div>",
    );
    let c = named(&ns, "c");
    assert_eq!((c.absolute, c.x, c.y), (true, 7.0, 8.0));
    assert!(!named(&ns, "b").absolute);
}

#[test]
fn right_bottom_fixed_sticky_and_a_relative_offset_are_refused() {
    let i = run(
        "<div id=a style=\"width:300px;position:relative\"><div id=b style=\"position:absolute;right:4px;bottom:4px;width:5px;height:5px\"></div><div id=c style=\"position:sticky;top:0;width:5px;height:5px\"></div><div id=d style=\"position:relative;top:3px;width:5px;height:5px\"></div></div>",
    );
    let ns = nodes(&i);
    let b = named(&ns, "b");
    assert_eq!((b.x, b.y), (0.0, 0.0));
    assert!(lost(&i, "b").contains(&"right".to_string()));
    assert!(lost(&i, "c").contains(&"position".to_string()));
    assert!(lost(&i, "d").contains(&"top".to_string()));
}

#[test]
fn overflow_decides_clip_and_the_model_default_is_never_left_to_chance() {
    let ns = one(
        "width:300px",
        "<div id=b style=\"overflow:hidden;width:10px;height:10px\"><p>x</p></div><div id=c style=\"width:10px;height:10px\"><p>x</p></div><div id=d style=\"overflow:auto;width:10px;height:10px\"><p>x</p></div>",
    );
    assert!(named(&ns, "b").clip);
    assert!(!named(&ns, "c").clip);
    assert!(named(&ns, "d").clip);
}

#[test]
fn grid_is_a_column_of_its_children_and_each_grid_property_is_listed() {
    let i = run(
        "<div id=a style=\"display:grid;grid-template-columns:1fr 1fr;grid-gap:4px;width:300px\"><div style=\"width:5px;height:5px\"></div><div style=\"width:5px;height:5px\"></div></div>",
    );
    let a = &nodes(&i)[0];
    assert_eq!(a.layout, LayoutMode::Column);
    let l = lost(&i, "a");
    assert!(
        l.contains(&"grid-template-columns".to_string()) && l.contains(&"grid-gap".to_string()),
        "{l:?}"
    );
}

#[test]
fn rem_and_em_are_converted_and_say_so() {
    let i = run("<div id=a style=\"width:20rem;padding:1em;font-size:10px;height:5px\"></div>");
    let a = &nodes(&i)[0];
    assert_eq!(
        (a.w, a.padding),
        (340.0, 10.0),
        "320 and the padding: content-box"
    );
    assert!(
        approx(&i, "a").contains(&"width".to_string()),
        "{:?}",
        info(&i, "a")
    );
}

#[test]
fn min_max_size_transform_z_index_and_float_are_refused_by_name() {
    let i = run(
        "<div id=a style=\"width:100px;max-width:50px;min-height:3px;transform:scale(2);z-index:3;float:left;line-height:1.5\"><p>x</p></div>",
    );
    let l = lost(&i, "a");
    for p in [
        "max-width",
        "min-height",
        "transform",
        "z-index",
        "float",
        "line-height",
    ] {
        assert!(l.contains(&p.to_string()), "{p} in {l:?}");
    }
    assert_eq!(nodes(&i)[0].w, 100.0);
}

#[test]
fn the_exports_own_declarations_are_not_refused() {
    // `data-node` marks an element of an export: its line-height pin and `flex-shrink:0` are part of the form.
    let i = run(
        "<div data-node=\"x\" data-name=\"Card\" style=\"display:flex;width:100px;height:20px\"><p data-node=\"y\" data-name=\"Label\" style=\"width:auto;min-width:30px;white-space:pre;word-break:keep-all;line-height:18px;flex-shrink:0\">Hi</p></div>",
    );
    assert_eq!(i.unsupported_count(), 0, "{:?}", i.nodes);
}

#[test]
fn text_kind_is_kept_for_a_text_that_is_the_whole_element() {
    let ns = one("width:100px", "<p style=\"margin:0\">x</p>");
    assert_eq!(ns[1].kind, NodeKind::Text);
}
