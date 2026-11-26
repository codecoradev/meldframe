//! The colour reader on its own: hex test, keywords, colour functions and channel triplets, as hex.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_css::color::{Hex, channels, function, is_hex, named};

fn hex(h: Option<Hex>) -> Option<(String, bool)> {
    h.map(|h| (h.hex, h.clipped))
}

#[test]
fn is_hex_takes_three_four_six_and_eight_digits_only() {
    for ok in ["#fff", "#FFFa", "#3b82f6", "#3B82F680"] {
        assert!(is_hex(ok), "{ok}");
    }
    for bad in [
        "fff",
        "#",
        "#ff",
        "#fffff",
        "#fffffff",
        "#ggg",
        "#fff ",
        "rgb(0 0 0)",
    ] {
        assert!(!is_hex(bad), "{bad}");
    }
}

#[test]
fn keywords_are_looked_up_without_regard_to_case() {
    assert_eq!(hex(named("white")), Some(("#ffffff".into(), false)));
    assert_eq!(hex(named("WHITE")), Some(("#ffffff".into(), false)));
    assert_eq!(hex(named("transparent")), Some(("#00000000".into(), false)));
    assert_eq!(hex(named("rebeccapurple")), None);
    assert_eq!(hex(named("currentColor")), None);
}

#[test]
fn rgb_hsl_oklch_and_oklab_become_hex() {
    for (css, want) in [
        ("rgb(255 0 0)", "#ff0000"),
        ("rgba(0, 0, 0, 0.5)", "#00000080"),
        ("rgb(0 0 0 / 50%)", "#00000080"),
        ("hsl(120deg 100% 25%)", "#008000"),
        ("hsl(120 50 50)", "#40bf40"),
        ("oklch(1 0 0)", "#ffffff"),
        ("oklch(0.5 -0.1 90)", "#636363"),
        ("oklab(1 0 0)", "#ffffff"),
    ] {
        assert_eq!(hex(function(css)), Some((want.into(), false)), "{css}");
    }
}

#[test]
fn a_colour_outside_srgb_is_clamped_and_flagged() {
    let got = function("oklch(0.7 0.4 150)").expect("a colour");
    assert!(got.clipped);
    assert!(
        got.hex.starts_with('#') && got.hex.len() == 7,
        "{}",
        got.hex
    );
    assert!(function("oklch(0.1 0.06 180)").expect("a colour").clipped);
}

#[test]
fn forms_that_are_not_read_give_none_never_a_colour() {
    for css in [
        "color(display-p3 1 0 0)",
        "lab(50% 40 59)",
        "rgb(1 2)",
        "rgb(1 2 3 4 5)",
        "rgb(var(--r) 0 0)",
        "oklch(NaN 1e999 inf)",
        "oklch(1e103 0 0)",
        "hsl(1e308 100% 50%)",
        "rgb(inf 0 0)",
        "red",
        "#fff",
    ] {
        assert_eq!(hex(function(css)), None, "{css}");
    }
}

#[test]
fn shadcn_channel_triplets_are_read_as_hsl() {
    assert_eq!(hex(channels("0 0% 100%")), Some(("#ffffff".into(), false)));
    assert_eq!(hex(channels("0 100% 50%")), Some(("#ff0000".into(), false)));
    assert_eq!(
        hex(channels("240 100% 50% / 0.5")),
        Some(("#0000ff80".into(), false))
    );
    assert_eq!(
        hex(channels("0 0 100")),
        None,
        "bare numbers are not a triplet"
    );
}
