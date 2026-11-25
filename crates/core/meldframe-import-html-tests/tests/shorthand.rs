//! The shorthands expanded to the longhands the mapping reads, so that the cascade can order each one on its own.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use html_import::css::shorthand::{expand, longhands};

/// `name:value` pairs of an expansion, in order.
fn ex(name: &str, value: &str) -> Vec<String> {
    expand(name, value)
        .unwrap_or_else(|| panic!("{name} is not a shorthand"))
        .unwrap_or_else(|e| panic!("{name}: {value} is refused: {e:?}"))
        .into_iter()
        .map(|(n, v)| format!("{n}:{v}"))
        .collect()
}

fn no(name: &str, value: &str) -> &'static str {
    expand(name, value).unwrap().unwrap_err().0
}

#[test]
#[rustfmt::skip]
fn the_four_sides_take_one_to_four_values() {
    assert_eq!(
        ex("margin", "1px"),
        [
            "margin-top:1px",
            "margin-right:1px",
            "margin-bottom:1px",
            "margin-left:1px"
        ]
    );
    assert_eq!(
        ex("padding", "1px 2px"),
        [
            "padding-top:1px",
            "padding-right:2px",
            "padding-bottom:1px",
            "padding-left:2px"
        ]
    );
    assert_eq!(
        ex("margin", "1px 2px 3px"),
        [
            "margin-top:1px",
            "margin-right:2px",
            "margin-bottom:3px",
            "margin-left:2px"
        ]
    );
    assert_eq!(
        ex("inset", "0 1px 2px 3px"),
        ["top:0", "right:1px", "bottom:2px", "left:3px"]
    );
    assert_eq!(
        ex("margin", "0 auto"),
        [
            "margin-top:0",
            "margin-right:auto",
            "margin-bottom:0",
            "margin-left:auto"
        ]
    );
    assert_eq!(
        ex("padding", "calc(1px + 2px) var(--x)")[0],
        "padding-top:calc(1px + 2px)"
    );
    assert_eq!(no("margin", "1 2 3 4 5"), "too_many_values");
    assert_eq!(no("margin", ""), "too_many_values");
}

#[test]
#[rustfmt::skip]
fn radius_gap_and_overflow_expand() {
    assert_eq!(
        ex("border-radius", "4px 8px"),
        [
            "border-top-left-radius:4px",
            "border-top-right-radius:8px",
            "border-bottom-right-radius:4px",
            "border-bottom-left-radius:8px"
        ]
    );
    assert_eq!(no("border-radius", "10px / 20px"), "elliptical_radius");
    assert_eq!(ex("gap", "4px"), ["row-gap:4px", "column-gap:4px"]);
    assert_eq!(ex("gap", "4px 8px"), ["row-gap:4px", "column-gap:8px"]);
    assert_eq!(
        ex("overflow", "hidden auto"),
        ["overflow-x:hidden", "overflow-y:auto"]
    );
    assert_eq!(
        ex("overflow", "clip"),
        ["overflow-x:clip", "overflow-y:clip"]
    );
    assert_eq!(no("gap", "1px 2px 3px"), "too_many_values");
}

#[test]
fn the_border_shorthands_take_width_style_and_colour_in_any_order() {
    let all = |w: &str, s: &str, c: &str| -> Vec<String> {
        ["width", "style", "color"]
            .iter()
            .zip([w, s, c])
            .flat_map(|(p, v)| {
                ["top", "right", "bottom", "left"].map(|side| format!("border-{side}-{p}:{v}"))
            })
            .collect()
    };
    assert_eq!(ex("border", "1px solid #000"), all("1px", "solid", "#000"));
    assert_eq!(ex("border", "#000 solid 1px"), all("1px", "solid", "#000"));
    assert_eq!(
        ex("border", "2px solid rgb(0 0 0 / 50%)"),
        all("2px", "solid", "rgb(0 0 0 / 50%)")
    );
    assert_eq!(ex("border", "none"), all("medium", "none", "currentcolor"));
    assert_eq!(ex("border", "0"), all("0", "none", "currentcolor"));
    assert_eq!(
        ex("border", "thick dashed"),
        all("thick", "dashed", "currentcolor")
    );
    assert_eq!(
        ex("border-top", "1px solid red"),
        [
            "border-top-width:1px",
            "border-top-style:solid",
            "border-top-color:red"
        ]
    );
    assert_eq!(
        ex("border-left", "solid"),
        [
            "border-left-width:medium",
            "border-left-style:solid",
            "border-left-color:currentcolor"
        ]
    );
    assert_eq!(
        ex("border-width", "1px 2px"),
        [
            "border-top-width:1px",
            "border-right-width:2px",
            "border-bottom-width:1px",
            "border-left-width:2px"
        ]
    );
    assert_eq!(ex("border-color", "red")[3], "border-left-color:red");
    assert_eq!(
        ex("border-style", "solid dashed")[1],
        "border-right-style:dashed"
    );
    assert_eq!(no("border", "1px 2px"), "border_shorthand");
    assert_eq!(no("border", "1px solid red blue"), "border_shorthand");
    assert_eq!(no("border", ""), "border_shorthand");
}

#[test]
fn flex_font_and_background_expand_as_far_as_the_note_reads_them() {
    for (v, want) in [
        ("1", "1 1 0%"),
        ("2", "2 1 0%"),
        ("none", "0 0 auto"),
        ("auto", "1 1 auto"),
        ("1 1 0%", "1 1 0%"),
        ("1 1 120px", "1 1 120px"),
        ("0 0 100px", "0 0 100px"),
        ("1 2", "1 2 0%"),
        ("3 40px", "3 1 40px"),
        ("100px", "1 1 100px"),
    ] {
        let g = ex("flex", v);
        let got = g
            .iter()
            .map(|s| s.split_once(':').unwrap().1)
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(got, want, "flex: {v}");
    }
    assert_eq!(no("flex", "1 1 1 1"), "flex_shorthand");
    assert_eq!(
        ex("font", "italic bold 12px/1.5 Inter, sans-serif"),
        [
            "font-style:italic",
            "font-weight:bold",
            "font-size:12px",
            "line-height:1.5",
            "font-family:Inter, sans-serif"
        ]
    );
    assert_eq!(
        ex("font", "500 1rem / 20px 'A B'"),
        [
            "font-style:normal",
            "font-weight:500",
            "font-size:1rem",
            "line-height:20px",
            "font-family:'A B'"
        ]
    );
    assert_eq!(no("font", "bold"), "font_shorthand");
    assert_eq!(no("font", "12px"), "font_shorthand");
    assert_eq!(no("font", "caption"), "font_shorthand");
    assert_eq!(
        ex("background", "#fff"),
        ["background-color:#fff", "background-image:none"]
    );
    assert_eq!(
        ex("background", "url(a.png) red"),
        ["background-color:red", "background-image:url(a.png)"]
    );
    assert_eq!(
        ex("background", "linear-gradient(90deg, red, blue)"),
        [
            "background-color:transparent",
            "background-image:linear-gradient(90deg, red, blue)"
        ]
    );
    assert_eq!(no("background", "red, blue"), "background_shorthand");
    assert_eq!(no("background", "url(a), url(b)"), "background_shorthand");
    assert_eq!(no("background", "red blue"), "background_shorthand");
}

#[test]
#[rustfmt::skip]
fn a_css_wide_keyword_goes_to_every_longhand_and_other_names_are_not_shorthands() {
    assert_eq!(ex("margin", "inherit")[2], "margin-bottom:inherit");
    assert_eq!(ex("border", "initial").len(), 12);
    assert_eq!(
        ex("flex", "unset"),
        ["flex-grow:unset", "flex-shrink:unset", "flex-basis:unset"]
    );
    for name in [
        "margin-top",
        "color",
        "display",
        "grid",
        "transition",
        "border-collapse",
        "--margin",
        "text-decoration",
    ] {
        assert!(expand(name, "1px").is_none(), "{name}");
        assert!(longhands(name).is_none(), "{name}");
    }
    assert_eq!(longhands("border").map(|l| l.len()), Some(12));
    assert_eq!(
        longhands("border-top"),
        Some(vec![
            "border-top-width",
            "border-top-style",
            "border-top-color"
        ])
    );
    // Every shorthand expands to exactly its longhands, in the same order.
    for (name, value) in [
        ("margin", "1px"),
        ("border", "1px solid red"),
        ("flex", "1"),
        ("font", "12px a"),
        ("background", "red"),
        ("gap", "1px"),
        ("overflow", "hidden"),
        ("border-radius", "1px"),
        ("border-right", "1px solid red"),
    ] {
        let names: Vec<_> = expand(name, value)
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(Some(names), longhands(name), "{name}");
    }
}
