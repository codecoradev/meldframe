//! `box-shadow`: outer shadows as the model holds them (offset, blur, spread, colour), in the model's order, and
//! what is refused.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::paint::Ctx;
use meldframe_protocol::Shadow;

fn shadow(x: f64, y: f64, blur: f64, spread: f64, color: &str) -> Shadow {
    Shadow {
        x,
        y,
        blur,
        spread,
        color: color.into(),
    }
}

fn sh(value: &str) -> Read {
    read(&format!("box-shadow:{value}"))
}

#[test]
fn a_shadow_is_offset_blur_spread_and_colour() {
    let r = sh("0 4px 8px rgba(0,0,0,.25)");
    assert_eq!(r.paint.shadows, [shadow(0.0, 4.0, 8.0, 0.0, "#00000040")]);
    assert!(r.report.entries.is_empty());
    assert_eq!(
        sh("1px 2px 3px 4px #ff0000").paint.shadows,
        [shadow(1.0, 2.0, 3.0, 4.0, "#ff0000")]
    );
    // The colour may come first; blur and spread are 0 when left out; offsets and spread may be negative.
    assert_eq!(
        sh("red 1px 2px").paint.shadows,
        [shadow(1.0, 2.0, 0.0, 0.0, "#ff0000")]
    );
    assert_eq!(
        sh("-1px -2px 0 -3px #000").paint.shadows,
        [shadow(-1.0, -2.0, 0.0, -3.0, "#000")]
    );
    assert_eq!(
        sh("0 0.5rem 1rem #111").paint.shadows,
        [shadow(0.0, 8.0, 16.0, 0.0, "#111")]
    );
}

#[test]
fn the_colour_left_out_is_the_colour_of_the_element() {
    let r = read("color:#336699;box-shadow:1px 2px 3px");
    assert_eq!(r.paint.shadows, [shadow(1.0, 2.0, 3.0, 0.0, "#336699")]);
    let r = read("color:#336699;box-shadow:1px 2px currentcolor");
    assert_eq!(r.paint.shadows[0].color, "#336699");
}

#[test]
fn a_list_is_kept_in_the_models_order_the_first_of_the_model_is_the_bottom() {
    let r = sh("1px 1px red, 2px 2px blue, 3px 3px lime");
    let xs: Vec<f64> = r.paint.shadows.iter().map(|s| s.x).collect();
    assert_eq!(xs, [3.0, 2.0, 1.0]);
    assert_eq!(r.paint.shadows[2].color, "#ff0000");
}

#[test]
fn none_and_no_declaration_are_no_shadow() {
    assert!(sh("none").paint.shadows.is_empty() && sh("none").report.entries.is_empty());
    assert!(read("").paint.shadows.is_empty());
}

#[test]
fn an_inner_shadow_is_refused_and_the_others_are_kept() {
    for value in [
        "inset 0 1px 2px red, 3px 3px blue",
        "0 1px 2px red inset, 3px 3px blue",
        "3px 3px blue, inset 0 1px 2px red",
    ] {
        let r = sh(value);
        assert_eq!(r.paint.shadows.len(), 1, "{value}");
        assert_eq!(r.paint.shadows[0].x, 3.0, "{value}");
        assert_eq!(r.notes(), ["refused:shadow_inset"], "{value}");
    }
}

#[test]
fn only_the_four_topmost_are_kept() {
    let list: Vec<String> = (1..=6).map(|i| format!("{i}px 0 red")).collect();
    let r = sh(&list.join(","));
    let xs: Vec<f64> = r.paint.shadows.iter().map(|s| s.x).collect();
    assert_eq!(xs, [4.0, 3.0, 2.0, 1.0]);
    assert_eq!(r.notes(), ["refused:shadow_extra", "refused:shadow_extra"]);
    // An inner shadow does not count against the four.
    let r = sh("inset 0 0 1px red, 1px 0 red, 2px 0 red, 3px 0 red, 4px 0 red");
    assert_eq!(r.paint.shadows.len(), 4);
    assert_eq!(r.notes(), ["refused:shadow_inset"]);
}

#[test]
fn what_is_not_a_shadow_is_refused_never_guessed() {
    for bad in [
        "banana",
        "1px",
        "1px red",
        "1px 2px 3px 4px 5px red",
        "red blue 1px 2px",
        "1px red 2px",
        "1px 2px red red",
        "1px 2px -3px red",
        "1% 2px red",
        "1px 2px 3px lab(50% 40 59)",
        "calc(1px + 2px) 2px red",
        "1000000px 0 0 red",
        "0 0 0 -1000000px red",
        "inset inset 1px 2px red",
    ] {
        let r = sh(bad);
        assert!(r.paint.shadows.is_empty(), "{bad}");
        assert!(
            r.notes()
                .iter()
                .all(|n| n == "refused:shadow_unreadable" || n == "refused:shadow_inset"),
            "{bad}: {:?}",
            r.notes()
        );
        assert_eq!(r.notes().len(), 1, "{bad}");
    }
    // A bad layer goes and the others stay.
    let r = sh("banana, 1px 1px red");
    assert_eq!(r.paint.shadows, [shadow(1.0, 1.0, 0.0, 0.0, "#ff0000")]);
    assert_eq!(r.notes(), ["refused:shadow_unreadable"]);
}

#[test]
fn a_list_past_the_bound_is_cut_and_says_so() {
    let list: Vec<String> = (0..40).map(|i| format!("{i}px 0 red")).collect();
    let r = sh(&list.join(","));
    assert_eq!(r.paint.shadows.len(), 4);
    assert!(r.has("cut:shadow_layers_cut"));
}

#[test]
fn a_var_that_holds_the_list_reads_the_same() {
    let r = read_in(
        "<div id=t style=\"box-shadow:var(--s)\">x</div>",
        ":root{--s:0 1px 2px #123456}",
        &Ctx::default(),
    );
    assert_eq!(r.paint.shadows, [shadow(0.0, 1.0, 2.0, 0.0, "#123456")]);
}
