//! Colour forms: what is kept, what is converted, what is clamped and what is refused.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use variables_import::value::{Conversion, Val, classify};

/// The colour a value reads as, and what was done to it.
fn colour(css: &str) -> (String, Option<Conversion>) {
    match classify("c", css) {
        Val::Color { hex, how } => (hex, how),
        other => panic!("{css} is not read as a colour: {other:?}"),
    }
}

fn hex_of(css: &str) -> String {
    colour(css).0
}

fn skipped(css: &str) -> &'static str {
    match classify("c", css) {
        Val::Skip(code, _) => code,
        other => panic!("{css} is not skipped: {other:?}"),
    }
}

#[test]
fn a_hex_colour_is_kept_as_written_and_not_a_conversion() {
    for h in ["#fff", "#FFFa", "#3b82f6", "#3B82F680"] {
        assert_eq!(colour(h), (h.to_string(), None));
    }
}

#[test]
fn rgb_hsl_and_keywords_become_hex_and_are_conversions() {
    let cases = [
        ("rgb(255 0 0)", "#ff0000"),
        ("rgb(255, 128, 0)", "#ff8000"),
        ("rgba(0, 0, 0, 0.5)", "#00000080"),
        ("rgb(0 0 0 / 50%)", "#00000080"),
        ("rgb(100% 0% 0%)", "#ff0000"),
        ("hsl(0 100% 50%)", "#ff0000"),
        ("hsl(120deg 100% 25%)", "#008000"),
        ("hsl(0.5turn 100% 50%)", "#00ffff"),
        ("hsla(240, 100%, 50%, 1)", "#0000ff"),
        ("white", "#ffffff"),
        ("WHITE", "#ffffff"),
        ("transparent", "#00000000"),
    ];
    for (css, hex) in cases {
        assert_eq!(
            colour(css),
            (hex.to_string(), Some(Conversion::Colour)),
            "{css}"
        );
    }
}

#[test]
fn oklch_and_oklab_convert_and_in_gamut_colours_are_not_clipped() {
    for (css, hex) in [
        ("oklch(1 0 0)", "#ffffff"),
        ("oklch(0 0 0)", "#000000"),
        ("oklch(0.5 0 0)", "#636363"),
        ("oklch(100% 0 0)", "#ffffff"),
        ("oklch(1 0 0 / 0.5)", "#ffffff80"),
        ("oklab(1 0 0)", "#ffffff"),
    ] {
        assert_eq!(
            colour(css),
            (hex.to_string(), Some(Conversion::Colour)),
            "{css}"
        );
    }
}

#[test]
fn an_oklch_colour_near_a_tailwind_red_lands_within_two_steps_of_its_hex() {
    // Tailwind v4 red-500 is oklch(63.7% 0.237 25.331); its sRGB is about #fb2c36.
    let got = hex_of("oklch(63.7% 0.237 25.331)");
    let ch = |i: usize| i64::from(u8::from_str_radix(&got[1 + 2 * i..3 + 2 * i], 16).unwrap());
    for (i, want) in [0xfb, 0x2c, 0x36].into_iter().enumerate() {
        assert!((ch(i) - want).abs() <= 2, "{got} channel {i}");
    }
}

#[test]
fn a_colour_outside_srgb_is_clamped_and_marked_clipped() {
    let (hex, how) = colour("oklch(0.7 0.4 150)");
    assert!(hex.starts_with('#') && hex.len() == 7, "{hex}");
    assert_eq!(how, Some(Conversion::Clipped));
}

#[test]
fn shadcn_channel_triplets_are_read_as_hsl() {
    assert_eq!(hex_of("0 0% 100%"), "#ffffff");
    assert_eq!(hex_of("0 100% 50%"), "#ff0000");
    assert_eq!(hex_of("240 100% 50% / 0.5"), "#0000ff80");
    assert_eq!(
        skipped("0 0 100"),
        "unsupported_type",
        "bare numbers are not a triplet"
    );
}

#[test]
fn forms_the_import_does_not_convert_are_skipped_with_a_reason() {
    for css in [
        "color(display-p3 1 0 0)",
        "lab(50% 40 59)",
        "hwb(0 0% 0%)",
        "rgb(1 2)",
        "oklch(x y z)",
        "rgb(1 2 3 4 5)",
    ] {
        assert_eq!(skipped(css), "unsupported_color", "{css}");
    }
    for css in [
        "color-mix(in oklab, red, blue)",
        "light-dark(#fff, #000)",
        "rgb(var(--r) 0 0)",
    ] {
        assert_eq!(skipped(css), "computed_value", "{css}");
    }
    assert_eq!(skipped("currentColor"), "unsupported_type");
}

#[test]
fn bare_numbers_in_hsl_are_percent_points() {
    assert_eq!(hex_of("hsl(120 50 50)"), "#40bf40");
    assert_eq!(hex_of("hsl(120 50% 50%)"), "#40bf40");
    assert_eq!(hex_of("hsl(0 100 50)"), "#ff0000");
}

#[test]
fn a_clamp_near_black_is_flagged_in_srgb_terms() {
    // Linear light is only -0.0019 here, which is about six steps of 8 bits once encoded.
    let (_, how) = colour("oklch(0.1 0.06 180)");
    assert_eq!(how, Some(Conversion::Clipped));
}

#[test]
fn numbers_that_overflow_or_are_not_numbers_are_skipped_never_painted() {
    for css in [
        "oklch(NaN 1e999 inf)",
        "oklch(1e103 0 0)",
        "oklab(1e103 0 0)",
        "oklch(1e308 1e308 1e308)",
        "oklch(0.5 1e308 90)",
        "hsl(1e308 100% 50%)",
        "hsl(nan 50% 50%)",
        "rgb(inf 0 0)",
        "oklch(1 0 infinity)",
    ] {
        assert_eq!(skipped(css), "unsupported_color", "{css}");
    }
    assert_eq!(
        colour("rgb(1e308 0 0)"),
        ("#ff0000".to_string(), Some(Conversion::Colour)),
        "CSS clamps a channel"
    );
}

#[test]
fn a_negative_chroma_is_grey_not_the_opposite_hue() {
    // CSS clamps chroma to 0; the hue then does not matter.
    for css in [
        "oklch(0.5 -0.1 90)",
        "oklch(0.5 -0.4 250)",
        "oklch(0.5 0 90)",
    ] {
        assert_eq!(hex_of(css), "#636363", "{css}");
    }
}
