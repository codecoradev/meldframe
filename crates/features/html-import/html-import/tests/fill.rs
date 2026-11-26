//! The fill of a box: `background-color` as the model holds it (a colour with its alpha, or `none`), and the words
//! that are not a colour.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "paint_common/mod.rs"]
mod common;
use common::*;
use html_import::paint::Paint;
use meldframe_protocol::Bind;

#[test]
fn a_box_with_no_declaration_paints_nothing_and_says_nothing() {
    let r = read("");
    assert_eq!(r.paint, Paint::default());
    assert_eq!(r.fill(), "none");
    assert!(r.report.entries.is_empty() && r.report.counts.is_empty());
}

#[test]
fn background_color_is_the_fill_in_hex_with_its_alpha() {
    for (css, hex) in [
        ("#ff0000", "#ff0000"),
        ("#F00", "#f00"),
        ("#AABBCC80", "#aabbcc80"),
        ("rgb(255, 0, 0)", "#ff0000"),
        ("rgba(0,0,0,.5)", "#00000080"),
        ("hsl(120 100% 50%)", "#00ff00"),
        ("RED", "#ff0000"),
    ] {
        let r = read(&format!("background-color:{css}"));
        assert_eq!(r.fill(), hex, "{css}");
        assert!(r.report.entries.is_empty(), "{css}: {:?}", r.notes());
    }
}

#[test]
fn transparent_is_no_fill_and_currentcolor_is_the_colour_of_the_element() {
    assert_eq!(read("background-color:transparent").fill(), "none");
    let r = read("color:#112233;background-color:currentcolor");
    assert_eq!(r.fill(), "#112233");
    // The colour is the one the element inherits.
    let r = read_in(
        "<p style=\"color:#336699\"><b id=t style=\"background-color:currentcolor\">x</b></p>",
        "",
        &Default::default(),
    );
    assert_eq!(r.fill(), "#336699");
}

#[test]
fn the_shorthand_gives_its_colour_and_a_fill_stays_a_literal() {
    let r = read("background:#0f0");
    assert_eq!(r.fill(), "#0f0");
    assert_eq!(r.paint.fill, Bind::Lit("#0f0".into()));
    // A later longhand beats the shorthand.
    assert_eq!(read("background:#0f0;background-color:#00f").fill(), "#00f");
}

#[test]
fn a_word_that_is_not_a_colour_is_refused_and_listed_never_guessed() {
    for bad in [
        "banana",
        "rebeccapurple",
        "lab(50% 40 59)",
        "color-mix(in srgb, red, blue)",
        "ButtonFace",
        "light-dark(red, blue)",
        "10px",
        "red blue",
        "#12",
        "url(a.png)",
    ] {
        let r = read(&format!("background-color:{bad}"));
        assert_eq!(r.fill(), "none", "{bad}");
        assert_eq!(r.notes(), ["refused:colour_unreadable"], "{bad}");
        assert!(
            r.report.entries[0]
                .message
                .contains(&bad[..bad.len().min(10)])
        );
    }
}

#[test]
fn a_shorthand_the_style_stage_refused_gives_no_paint_and_no_second_line() {
    // `no-repeat` is no colour and no image: the shorthand is refused as a whole by the style stage, which lists it
    // on the element (`Style::notes`); the paint stage does not say it again.
    let r = read("background:no-repeat #f00");
    assert_eq!(r.paint, Paint::default());
    assert!(
        r.style
            .notes
            .iter()
            .any(|n| n.code == "background_shorthand")
    );
    assert!(r.report.entries.is_empty());
}

#[test]
fn a_colour_outside_srgb_is_clamped_and_listed() {
    let r = read("background-color:oklch(0.7 0.4 30)");
    assert!(
        r.fill().starts_with('#') && r.fill() != "none",
        "{}",
        r.fill()
    );
    assert_eq!(r.notes(), ["listed:colour_clipped"]);
    // One inside it is exact and says nothing.
    let r = read("background-color:oklch(0.5 0.05 30)");
    assert!(r.fill().starts_with('#') && r.report.entries.is_empty());
}

#[test]
fn background_color_declared_by_a_rule_and_a_var_reads_the_same() {
    let r = read_in(
        "<div id=t class=a>x</div>",
        ":root{--c:#123456} .a{background-color:var(--c)}",
        &Default::default(),
    );
    assert_eq!(r.fill(), "#123456");
    // A var() that did not resolve unsets the property (the style stage says why): no fill, no line here.
    let r = read("background-color:var(--missing)");
    assert_eq!(r.fill(), "none");
    assert!(r.report.entries.is_empty());
}
