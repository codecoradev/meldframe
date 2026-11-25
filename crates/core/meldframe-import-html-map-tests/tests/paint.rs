//! The mapping calls the paint stage for every node it makes and puts the answer on the node (`docs/import-html.md`
//! section 3.6; the reading itself is tested in `meldframe-import-html-paint-tests`).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::map::Options;
use html_import::map::plan;
use meldframe_protocol::{Bind, BorderPosition, NodeKind, SizeMode};

#[test]
fn a_frame_takes_the_fill_radius_and_shadow_of_its_style() {
    let list = map(
        "<div data-name=Box style='background-color:#336699;border-radius:6px;box-shadow:1px 2px 3px 0 #000'>\
         <p>a</p><p>b</p></div>",
    );
    let b = named(&list, "Box");
    assert_eq!(b.fill, Bind::Lit("#336699".into()));
    assert_eq!(b.radius, Bind::Lit(6.0));
    assert_eq!((b.shadows.len(), b.shadows[0].blur), (1, 3.0));
}

#[test]
fn a_text_that_paints_a_box_is_a_frame_with_a_text_and_the_text_keeps_only_its_colour_and_opacity()
{
    let list = map(
        "<div data-name=Box><p data-name=P style='background:#eee;color:#123456;opacity:.5'>x</p><p>y</p></div>",
    );
    let p = named(&list, "P");
    assert_eq!(p.kind, NodeKind::Frame);
    assert_eq!(p.fill, Bind::Lit("#eee".into()));
    let t = list
        .iter()
        .find(|n| n.parent_id.as_deref() == Some(p.id.as_str()))
        .unwrap();
    assert_eq!(
        (t.kind, t.fill.clone(), t.opacity.clone()),
        (NodeKind::Text, Bind::Lit("#123456".into()), Bind::Lit(1.0))
    );
    assert_eq!(p.opacity, Bind::Lit(0.5));
}

#[test]
fn an_opacity_alone_does_not_make_a_text_a_frame() {
    let list = map("<div data-name=Box><p data-name=P style='opacity:.5'>x</p><p>y</p></div>");
    let p = named(&list, "P");
    assert_eq!(
        (p.kind, p.opacity.clone()),
        (NodeKind::Text, Bind::Lit(0.5))
    );
}

#[test]
fn a_css_border_takes_room_from_the_padding_and_the_size() {
    let list = map(
        "<div data-name=Box style='border:2px solid #000;padding:3px;width:100px;height:50px'><p>a</p><p>b</p></div>",
    );
    let b = named(&list, "Box");
    assert_eq!(b.border.width, 2.0);
    let pad = b.pad();
    assert_eq!((pad.top, pad.left), (5.0, 5.0));
    assert_eq!((b.w, b.h), (110.0, 60.0));
    let list = map(
        "<div data-name=Box style='box-sizing:border-box;border:2px solid #000;width:100px;height:50px'><p>a</p><p>b</p></div>",
    );
    let b = named(&list, "Box");
    assert_eq!((b.w, b.h, b.pad().top), (100.0, 50.0, 2.0));
}

#[test]
fn the_border_the_export_writes_takes_no_room() {
    let css = "[data-node=\"b\"]::after{content:\"\";position:absolute;pointer-events:none;inset:0;border:2px solid #000}";
    let list = nodes(&planned(
        "<div data-node=b data-name=Box style='width:100px;height:50px'><p>a</p><p>b</p></div>",
        &format!("{RESET}{css}"),
    ));
    let b = named(&list, "Box");
    assert_eq!(
        (b.border.width, b.border.position),
        (2.0, BorderPosition::Inside)
    );
    assert_eq!((b.pad().top, b.w, b.h), (0.0, 100.0, 50.0));
}

#[test]
fn a_round_box_with_nothing_inside_is_an_ellipse_and_with_something_inside_a_frame() {
    let list = map(
        "<div data-name=Top><div data-name=E style='width:40px;height:40px;border-radius:50%'></div>\
         <div data-name=F style='width:40px;height:40px;border-radius:50%'>x</div>\
         <div data-name=G style='width:40px;height:40px;border-radius:50%'><div><p>y</p><p>z</p></div></div>\
         <div data-name=H style='width:40px;height:40px;padding:10px;border-radius:50%'><p>a</p><p>b</p></div></div>",
    );
    let e = named(&list, "E");
    assert_eq!(
        (e.kind, e.radius.clone()),
        (NodeKind::Ellipse, Bind::Lit(0.0))
    );
    let f = named(&list, "F");
    assert_eq!(
        (f.kind, f.radius.clone()),
        (NodeKind::Frame, Bind::Lit(20.0))
    );
    assert_eq!(
        named(&list, "H").radius,
        Bind::Lit(30.0),
        "half of the border box, not the content"
    );
    let g = named(&list, "G");
    assert_eq!(
        (g.kind, g.radius.clone()),
        (NodeKind::Frame, Bind::Lit(20.0))
    );
}

#[test]
fn an_image_and_what_the_model_cannot_hold_paint_nothing_and_the_src_is_kept() {
    let p = planned(
        "<div data-name=Box><img data-name=I src='https://x.test/a.png' style='background:red;width:10px;height:10px'>\
         <svg data-name=S style='background:red;width:5px;height:5px'></svg></div>",
        RESET,
    );
    let list = nodes(&p);
    assert_eq!(named(&list, "I").fill, Bind::Lit("none".into()));
    assert_eq!(named(&list, "S").fill, Bind::Lit("none".into()));
    assert_eq!(p.assets[0].src, "https://x.test/a.png");
}

#[test]
fn what_paint_refuses_is_in_the_report_once_with_the_byte_of_the_element() {
    let p = planned(
        "<div data-name=Box style='filter:blur(2px)'><p>a</p><p>b</p></div>",
        RESET,
    );
    let e: Vec<_> = p
        .report
        .entries
        .iter()
        .filter(|e| e.name == "filter")
        .collect();
    assert_eq!(e.len(), 1, "{:?}", p.report.entries);
    assert_eq!(e[0].at, p.nodes[0].at);
    assert!(
        p.nodes[0]
            .unsupported
            .iter()
            .all(|u| u.property != "filter")
    );
}

#[test]
fn a_wrapper_with_a_background_stays_and_paints() {
    let list = map(
        "<div data-name=Top><div class=back style='background:#fff'><p>a</p></div><p>b</p></div>",
    );
    assert_eq!(named(&list, "back").fill, Bind::Lit("#fff".into()));
}

#[test]
fn the_body_paint_is_the_roots() {
    let o = Options::default();
    let list = nodes(
        &plan(
            "<body style='background:#101010;margin:0'><div><p>a</p><p>b</p></div></body>",
            "",
            &o,
        )
        .unwrap(),
    );
    assert_eq!(list[0].fill, Bind::Lit("#101010".into()));
    assert_eq!(list[0].sw, SizeMode::Fixed);
}
