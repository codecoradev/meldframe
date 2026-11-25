//! Sizing: fixed, hug and fill (`docs/import-html.md` section 3.4, the width and height rows).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::map::{Options, plan};
use meldframe_protocol::SizeMode::{self, Fill, Fixed, Hug};

/// The modes `(sw, sh)` of the node `P`, a `<div data-name=P style=child>` inside `<div data-name=Box style=parent>`
/// that also holds a second child.
fn modes(parent: &str, child: &str) -> (SizeMode, SizeMode) {
    let list = map(&format!(
        "<div data-name=Box style='{parent}'><div data-name=P style='{child}'><p>x</p><p>y</p></div><div data-name=Q><p>z</p><p>w</p></div></div>"
    ));
    let n = named(&list, "P");
    (n.sw, n.sh)
}

fn size(child: &str) -> (f64, f64) {
    let list = map(&format!(
        "<div data-name=Box style='display:flex'><div data-name=P style='{child}'><p>x</p><p>y</p></div><div><p>z</p><p>q</p></div></div>"
    ));
    let n = named(&list, "P");
    (n.w, n.h)
}

#[test]
fn a_size_in_pixels_is_fixed() {
    let list = map(
        "<div data-name=Box style='display:flex'><div data-name=P style='width:100px;height:50px'><p>x</p><p>y</p></div><p>z</p></div>",
    );
    let p = named(&list, "P");
    assert_eq!((p.sw, p.sh, p.w, p.h), (Fixed, Fixed, 100.0, 50.0));
}

#[test]
fn content_box_adds_the_padding_to_a_fixed_size_and_border_box_does_not() {
    assert_eq!(
        size("width:100px;height:40px;padding:10px 5px"),
        (110.0, 60.0)
    );
    assert_eq!(
        size("box-sizing:border-box;width:100px;height:40px;padding:10px 5px"),
        (100.0, 40.0)
    );
    let p = planned(
        "<div data-name=Box style='display:flex'><div data-name=P style='width:100px;padding:5px'><p>x</p><p>y</p></div></div>",
        RESET,
    );
    let approx: Vec<&str> = p.nodes[1]
        .approximated
        .iter()
        .map(|a| a.property.as_str())
        .collect();
    assert_eq!(approx, ["box-sizing"]);
}

#[test]
fn auto_width_follows_the_parent_a_row_hugs_and_a_column_or_block_fills() {
    assert_eq!(
        modes("display:flex;height:80px", ""),
        (Hug, Fill),
        "row: hug the width, stretch the height"
    );
    assert_eq!(modes("display:flex;flex-direction:column", ""), (Fill, Hug));
    assert_eq!(modes("display:block", ""), (Fill, Hug));
    assert_eq!(
        modes("display:flex;align-items:flex-start;height:80px", ""),
        (Hug, Hug)
    );
    assert_eq!(
        modes("display:flex;flex-direction:column;align-items:center", ""),
        (Hug, Hug)
    );
}

#[test]
fn a_hugging_parent_makes_its_auto_children_hug() {
    assert_eq!(modes("display:block;width:fit-content", ""), (Hug, Hug));
    assert_eq!(
        modes("display:flex;width:300px", ""),
        (Hug, Hug),
        "a row with no height hugs, so nothing stretches"
    );
    assert_eq!(
        modes("display:flex;width:300px;height:80px", ""),
        (Hug, Fill)
    );
}

#[test]
fn fit_content_and_max_content_hug_and_min_content_hugs_approximated() {
    assert_eq!(
        modes("display:flex", "width:fit-content;height:max-content"),
        (Hug, Hug)
    );
    let p = planned(
        "<div data-name=Box style='display:flex'><div data-name=P style='width:min-content'><p>x</p><p>y</p></div></div>",
        RESET,
    );
    assert_eq!(p.nodes[1].approximated[0].property, "width");
}

#[test]
fn flex_grow_fills_the_main_axis() {
    assert_eq!(modes("display:flex;height:80px", "flex:1"), (Fill, Fill));
    assert_eq!(
        modes("display:flex;height:80px", "flex-grow:1"),
        (Fill, Fill)
    );
    assert_eq!(
        modes("display:flex;height:50px", "flex:1 1 24px;min-width:0"),
        (Fill, Fill)
    );
    assert_eq!(
        modes("display:flex;flex-direction:column;height:200px", "flex:1"),
        (Fill, Fill)
    );
    assert_eq!(
        modes("display:flex;height:50px", "flex-grow:0"),
        (Hug, Fill)
    );
    let p = planned(
        "<div data-name=Box style='display:flex'><div data-name=P style='flex-grow:2'><p>x</p><p>y</p></div></div>",
        RESET,
    );
    assert_eq!(p.nodes[1].approximated[0].property, "flex-grow");
}

#[test]
fn a_fill_under_a_hugging_parent_hugs() {
    assert_eq!(
        modes("display:flex;width:fit-content;height:50px", "flex:1"),
        (Hug, Fill)
    );
    assert_eq!(
        modes(
            "display:flex;width:fit-content;height:50px",
            "flex:1 1 24px"
        ),
        (Hug, Fill),
        "a basis does not make a fixed size of a fill"
    );
    assert_eq!(
        modes("display:flex;flex-direction:column", "flex:1"),
        (Fill, Hug),
        "a column with no height hugs"
    );
}

#[test]
fn a_basis_in_pixels_with_no_grow_is_the_size() {
    assert_eq!(size("flex:0 0 50px").0, 50.0);
    assert_eq!(size("flex:none;width:30px").0, 30.0);
}

#[test]
fn stretch_is_the_cross_size_of_the_default_and_of_align_self() {
    assert_eq!(
        modes(
            "display:flex;align-items:center;height:80px",
            "align-self:stretch"
        ),
        (Hug, Fill)
    );
    assert_eq!(
        modes("display:flex;align-items:center;height:80px", ""),
        (Hug, Hug)
    );
    let p = planned(
        "<div data-name=Box style='display:flex;align-items:center'><div data-name=P style='align-self:flex-end'><p>x</p><p>y</p></div></div>",
        RESET,
    );
    assert_eq!(p.nodes[1].unsupported[0].property, "align-self");
}

#[test]
fn a_hundred_percent_fills_a_parent_that_does_not_hug_else_the_parents_pixels() {
    assert_eq!(modes("display:block", "width:100%"), (Fill, Hug));
    assert_eq!(
        modes(
            "display:flex;width:300px;height:200px",
            "width:100%;height:100%"
        ),
        (Fill, Fill)
    );
    let list = map(
        "<div data-name=Box style='display:flex;flex-direction:column;width:300px'><div data-name=P style='height:50px'><p>x</p><p>y</p></div></div>",
    );
    assert_eq!(named(&list, "P").sw, Fill);
    let p = planned(
        "<div data-name=Box style='width:400px'><div data-name=P style='width:50%'><p>x</p><p>y</p></div></div>",
        RESET,
    );
    assert_eq!(p.nodes[1].unsupported[0].code, "refused_value");
}

#[test]
fn min_and_max_sizes_are_refused_and_zero_or_the_text_pin_is_not() {
    let p = planned(
        "<div data-name=Box style='display:flex'><div data-name=P style='max-width:200px;min-height:10px'><p>x</p><p>y</p></div>\
         <div data-name=Z style='min-width:0;max-width:none;min-height:auto'><p>x</p><p>y</p></div></div>",
        RESET,
    );
    let props = |n: &str| -> Vec<&str> {
        p.nodes
            .iter()
            .find(|x| x.name == n)
            .unwrap()
            .unsupported
            .iter()
            .map(|u| u.property.as_str())
            .collect()
    };
    assert_eq!(props("P"), ["min-height", "max-width"]);
    assert!(props("Z").is_empty());
}

#[test]
fn a_text_is_as_wide_as_its_line_when_it_cannot_wrap_else_it_fills_or_hugs_by_its_parent() {
    let t = |parent: &str, own: &str| {
        let list = map(&format!(
            "<div data-name=Box style='{parent}'><p data-name=T style='{own}'>word word</p><p>b</p></div>"
        ));
        let n = named(&list, "T");
        (n.sw, n.sh)
    };
    assert_eq!(t("display:block", ""), (Fill, Hug));
    assert_eq!(t("display:flex", ""), (Hug, Hug));
    assert_eq!(
        t("display:flex;height:80px", ""),
        (Hug, Hug),
        "a text is as tall as its lines, a stretching row does not make it taller"
    );
    assert_eq!(t("display:block", "white-space:nowrap"), (Hug, Hug));
    assert_eq!(t("display:block", "white-space:pre"), (Hug, Hug));
    assert_eq!(
        t("display:block", "width:auto;min-width:40px"),
        (Hug, Hug),
        "the pin of the export says hug"
    );
    assert_eq!(t("display:block", "width:120px"), (Fixed, Hug));
    assert_eq!(t("display:block;width:fit-content", ""), (Hug, Hug));
}

#[test]
fn the_root_is_as_wide_as_the_css_says_else_the_option() {
    let w = |html: &str, css: &str, width: f64| {
        let o = Options {
            width,
            ..Options::default()
        };
        let list = nodes(&plan(html, css, &o).unwrap());
        (list[0].sw, list[0].w, list[0].sh)
    };
    let two = "<div data-name=A style='width:800px'><p>a</p><p>b</p></div><div><p>c</p></div>";
    assert_eq!(w(two, RESET, 1440.0), (Fixed, 1440.0, Hug));
    let one = "<div data-name=A style='width:800px'><p>a</p><p>b</p></div>";
    assert_eq!(w(one, RESET, 1440.0), (Fixed, 800.0, Hug));
    let auto = "<div data-name=A><p>a</p><p>b</p></div>";
    assert_eq!(w(auto, RESET, 1000.0), (Fixed, 1000.0, Hug));
    let full = "<div data-name=A style='width:100%;height:300px'><p>a</p><p>b</p></div>";
    assert_eq!(w(full, RESET, 900.0), (Fixed, 900.0, Fixed));
    let fit = "<div data-name=A style='width:fit-content'><p>a</p><p>b</p></div>";
    assert_eq!(w(fit, RESET, 900.0).0, Hug);
}

#[test]
fn a_percentage_under_a_hugging_parent_is_refused_and_hugs() {
    let list = map(
        "<div data-name=Box style='display:block;width:fit-content'><div data-name=P style='width:100%'><p>x</p><p>y</p></div></div>",
    );
    let p = named(&list, "P");
    assert_eq!((p.sw, p.sh), (Hug, Hug));
    let p = planned(
        "<div data-name=Box style='display:block;width:fit-content'><div data-name=P style='width:100%'><p>x</p><p>y</p></div></div>",
        RESET,
    );
    assert_eq!(p.nodes[1].unsupported[0].property, "width");
}

#[test]
fn an_align_self_that_moves_a_box_is_refused_and_does_not_stretch_it() {
    let (parent, own) = ("display:flex;height:80px", "align-self:center");
    assert_eq!(modes(parent, own), (Hug, Hug));
    assert_eq!(modes(parent, "align-self:auto"), (Hug, Fill));
}

#[test]
fn the_cells_of_a_row_share_it_equally() {
    let list = map(
        "<table data-name=T><tr data-name=R><td data-name=A>a</td><td data-name=B>b</td></tr></table>",
    );
    assert_eq!(
        named(&list, "R").layout,
        meldframe_protocol::LayoutMode::Row
    );
    assert_eq!((named(&list, "A").sw, named(&list, "B").sw), (Fill, Fill));
}
