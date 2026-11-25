//! Text: content, white space, typography and the text a run of inline elements makes (`docs/import-html.md` section
//! 3.5).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_protocol::{Bind, TextAlign};

/// The text node `T` of `<p data-name=T style=...>content</p>` in a frame.
fn text(style: &str, content: &str) -> meldframe_protocol::Node {
    let list = map(&format!(
        "<div data-name=Box><p data-name=T style='{style}'>{content}</p><p>other</p></div>"
    ));
    named(&list, "T").clone()
}

#[test]
fn a_line_of_text_has_its_white_space_collapsed_and_inline_elements_joined() {
    let list = map("<p data-name=P>  Hello \n\t  <b>big</b>   <i>world</i>  <br> next  <br>  </p>");
    assert_eq!(named(&list, "P").text, "Hello big world\nnext");
    // A lone text is not a frame, so the page makes the root around it.
    assert_eq!((list.len(), list[0].name.as_str()), (2, "Imported HTML"));
}

#[test]
fn text_around_a_box_is_an_anonymous_text_in_the_frame() {
    let list = map("<div data-name=Box>before<div data-name=Mid>m</div>after</div>");
    let texts: Vec<(&str, &str)> = list
        .iter()
        .map(|n| (n.name.as_str(), n.text.as_str()))
        .collect();
    assert_eq!(
        texts,
        [
            ("Box", ""),
            ("Box text", "before"),
            ("Mid", "m"),
            ("Box text", "after")
        ]
    );
}

#[test]
fn the_size_weight_colour_and_alignment_are_the_texts() {
    let t = text(
        "font-size:20px;font-weight:700;color:#336699;text-align:center",
        "x",
    );
    assert_eq!(
        (t.font_size, t.font_weight, t.text_align),
        (20.0, 700, TextAlign::Center)
    );
    assert_eq!(t.fill, Bind::Lit("#336699".into()));
    let t = text(
        "color:rgba(0,0,0,0.5);text-align:right;font-weight:bold",
        "x",
    );
    assert_eq!(t.fill, Bind::Lit("#00000080".into()));
    assert_eq!((t.text_align, t.font_weight), (TextAlign::Right, 700));
    let t = text("", "x");
    assert_eq!(
        (t.font_size, t.font_weight, t.text_align),
        (16.0, 400, TextAlign::Left)
    );
    assert_eq!(
        t.fill,
        Bind::Lit("#000000".into()),
        "the model's grey is not CSS's black"
    );
}

#[test]
fn what_a_frame_sets_is_inherited_by_its_text_and_the_browsers_headings_are_big_and_bold() {
    let list = map(
        "<div data-name=Box style='font-size:18px;color:#f00'>bare<p data-name=P>a</p><h1 data-name=H>b</h1></div>",
    );
    let (bare, p, h) = (
        named(&list, "Box text"),
        named(&list, "P"),
        named(&list, "H"),
    );
    assert_eq!(
        (bare.font_size, bare.fill.clone()),
        (18.0, Bind::Lit("#f00".into()))
    );
    assert_eq!(p.font_size, 18.0);
    assert_eq!((h.font_size, h.font_weight), (36.0, 700));
}

#[test]
fn a_weight_between_the_steps_is_the_nearest_step_and_listed() {
    let w = |v: &str| text(&format!("font-weight:{v}"), "x").font_weight;
    assert_eq!(
        (w("450"), w("349"), w("1000"), w("1"), w("normal")),
        (500, 300, 900, 100, 400)
    );
    let p = planned(
        "<div data-name=Box><p data-name=T style='font-weight:450'>x</p><p>y</p></div>",
        RESET,
    );
    let t = p.nodes.iter().find(|n| n.name == "T").unwrap();
    assert_eq!(
        (
            t.approximated[0].property.as_str(),
            t.approximated[0].to.as_str()
        ),
        ("font-weight", "500")
    );
}

#[test]
fn start_end_and_justify_are_left_to_right_approximations() {
    let a = |v: &str| text(&format!("text-align:{v}"), "x").text_align;
    assert_eq!(
        (a("start"), a("end"), a("justify"), a("left")),
        (
            TextAlign::Left,
            TextAlign::Right,
            TextAlign::Left,
            TextAlign::Left
        )
    );
    let p = planned(
        "<div data-name=Box><p data-name=T style='text-align:end'>x</p><p>y</p></div>",
        RESET,
    );
    assert_eq!(
        p.nodes.iter().find(|n| n.name == "T").unwrap().approximated[0].property,
        "text-align"
    );
    let p = planned(
        "<div data-name=Box><p data-name=T style='text-align:match-parent'>x</p><p>y</p></div>",
        RESET,
    );
    assert_eq!(
        p.nodes.iter().find(|n| n.name == "T").unwrap().unsupported[0].code,
        "refused_value"
    );
}

#[test]
fn a_font_size_the_model_cannot_hold_is_kept_to_its_bounds_and_listed() {
    assert_eq!(text("font-size:0.2px", "x").font_size, 1.0);
    assert_eq!(text("font-size:200000px", "x").font_size, 100_000.0);
    let p = planned(
        "<div data-name=Box><p data-name=T style='font-size:0.2px'>x</p><p>y</p></div>",
        RESET,
    );
    assert_eq!(
        p.nodes.iter().find(|n| n.name == "T").unwrap().approximated[0].property,
        "font-size"
    );
}

#[test]
fn white_space_normal_and_nowrap_collapse_pre_and_pre_wrap_keep_and_pre_line_keeps_the_lines() {
    let c = |ws: &str| text(&format!("white-space:{ws}"), "a  b\n  c<br>d").text;
    assert_eq!(c("normal"), "a b c\nd");
    assert_eq!(c("nowrap"), "a b c\nd");
    assert_eq!(c("pre"), "a  b\n  c\nd");
    assert_eq!(c("pre-wrap"), "a  b\n  c\nd");
    assert_eq!(c("pre-line"), "a b\nc\nd");
    let p = planned(
        "<div data-name=Box><p data-name=T style='white-space:break-spaces'>x</p><p>y</p></div>",
        RESET,
    );
    assert_eq!(
        p.nodes.iter().find(|n| n.name == "T").unwrap().unsupported[0].property,
        "white-space"
    );
}

#[test]
fn text_transform_is_applied_to_the_characters_and_listed() {
    let t = |v: &str| text(&format!("text-transform:{v}"), "hello big world").text;
    assert_eq!(
        (t("uppercase"), t("lowercase"), t("capitalize"), t("none")),
        (
            "HELLO BIG WORLD".into(),
            "hello big world".into(),
            "Hello Big World".into(),
            "hello big world".into()
        )
    );
    let quoted = text("text-transform:capitalize", "\"hello (big) world");
    assert_eq!(
        quoted.text, "\"Hello (Big) World",
        "the first letter of a word, not its first character"
    );
    let p = planned(
        "<div data-name=Box><p data-name=T style='text-transform:uppercase'>x</p><p>y</p></div>",
        RESET,
    );
    assert_eq!(
        p.nodes.iter().find(|n| n.name == "T").unwrap().approximated[0].property,
        "text-transform"
    );
}

#[test]
fn inline_elements_of_another_style_are_joined_and_the_first_difference_is_listed() {
    let p = planned(
        "<div data-name=Box><p data-name=T>a <b>b</b> <span style='color:#f00'>c</span></p><p>y</p></div>",
        RESET,
    );
    let t = p.nodes.iter().find(|n| n.name == "T").unwrap();
    assert_eq!(
        t.approximated.len(),
        1,
        "once for the node: {:?}",
        t.approximated
    );
    assert_eq!(
        (
            t.approximated[0].property.as_str(),
            t.approximated[0].from.as_str(),
            t.approximated[0].to.as_str()
        ),
        ("font-weight", "700", "400")
    );
    let colour = planned(
        "<div data-name=Box><p data-name=T>a <span style='color:#f00'>c</span></p><p>y</p></div>",
        RESET,
    );
    let c = &colour
        .nodes
        .iter()
        .find(|n| n.name == "T")
        .unwrap()
        .approximated;
    assert_eq!(
        (c[0].property.as_str(), c[0].from.as_str()),
        ("color", "#f00")
    );
    let big = planned(
        "<div data-name=Box><p data-name=T>a <span style='font-size:30px'>c</span></p><p>y</p></div>",
        RESET,
    );
    assert_eq!(
        big.nodes
            .iter()
            .find(|n| n.name == "T")
            .unwrap()
            .approximated[0]
            .property,
        "font-size"
    );
    let same = planned(
        "<div data-name=Box><p data-name=T>a <span>b</span></p><p>y</p></div>",
        RESET,
    );
    assert!(
        same.nodes
            .iter()
            .find(|n| n.name == "T")
            .unwrap()
            .approximated
            .is_empty()
    );
}

#[test]
fn text_the_canvas_and_the_browser_break_differently_is_listed() {
    let p = planned(
        "<div data-name=Box><p>see https://x.test</p><p style='white-space:pre'>tab\there</p><p>no tab\there</p><p>日本語</p><p>plain</p></div>",
        RESET,
    );
    // A tab that collapses to a space is no problem: only a kept one is.
    assert_eq!(count(&p, "text_outside_contract"), 3);
}
