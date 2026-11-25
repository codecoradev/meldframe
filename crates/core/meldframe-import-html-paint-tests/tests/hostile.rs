//! Hostile values: the reader is given raw text of any shape, with no CSS reader in front of it, and must neither
//! panic nor do more work than the text is long.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::css::MAX_ENTRIES;
use html_import::paint::Ctx;

/// The properties this stage reads, each with a value that is valid, so that its prefixes are plausible.
const VALID: &[(&str, &str)] = &[
    ("background-color", "rgba(0, 0, 0, 0.5)"),
    (
        "background-image",
        "url('assets/abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789'), linear-gradient(to top right, red 0%, rgb(1 2 3) 50% 60%, #00f 100%), radial-gradient(circle farthest-corner, red, blue)",
    ),
    ("background-size", "cover, 100% 100%"),
    ("background-position", "center"),
    ("background-repeat", "no-repeat"),
    ("border-top-width", "calc(1px + 2px)"),
    ("border-top-style", "solid"),
    ("border-top-color", "oklch(0.7 0.4 30)"),
    (
        "border-top-left-radius",
        "calc(min(var(--r) * 1px, 50px, 20px) + 1px) 5px",
    ),
    (
        "box-shadow",
        "inset 0 1px 2px 3px rgba(0,0,0,.5), 1px 2px red, 3px 4px blue",
    ),
    ("opacity", "50%"),
    ("filter", "blur(2px)"),
    ("cursor", "pointer"),
];
const AFTER: &[(&str, &str)] = &[
    ("content", "\"\""),
    ("position", "absolute"),
    ("inset", "-1.5px"),
    ("top", "-1.5px"),
    ("border-top-width", "3px"),
    ("border-top-style", "solid"),
    ("border-top-color", "#ff0000"),
    (
        "border-top-left-radius",
        "calc(min(6 * 1px, 50px, 20px) + 1.5px)",
    ),
];

fn bounded(decls: &[(&str, &str)], after: &[(&str, &str)], ctx: &Ctx) {
    let s = style_with_after(decls, after);
    let (p, r) = paint_of(&s, ctx);
    assert!(r.entries.len() <= MAX_ENTRIES);
    assert!(p.shadows.len() <= 4 && p.assets.len() <= 16);
    assert!(
        p.gradient
            .as_ref()
            .is_none_or(|g| (2..=8).contains(&g.stops.len()))
    );
    // And again: the same text gives the same paint and the same report.
    let (p2, r2) = paint_of(&s, ctx);
    assert_eq!(p, p2);
    assert_eq!(r.entries, r2.entries);
}

#[test]
fn every_prefix_of_every_valid_value_is_read_without_a_panic() {
    let ctx = Ctx {
        width: Some(40.0),
        height: Some(20.0),
        has_children: true,
    };
    for (i, (name, value)) in VALID.iter().enumerate() {
        for end in (0..=value.len()).filter(|e| value.is_char_boundary(*e)) {
            let cut = &value[..end];
            let mut decls: Vec<(&str, &str)> = VALID.to_vec();
            decls[i] = (name, cut);
            bounded(&decls, &[], &ctx);
        }
    }
    for (i, (name, value)) in AFTER.iter().enumerate() {
        for end in (0..=value.len()).filter(|e| value.is_char_boundary(*e)) {
            let mut after: Vec<(&str, &str)> = AFTER.to_vec();
            after[i] = (name, &value[..end]);
            bounded(&[("border-radius", "8px")], &after, &ctx);
        }
    }
}

#[test]
fn a_corpus_of_nasty_text_in_every_property_is_refused_or_read_and_never_panics() {
    let nasty = [
        "",
        " ",
        "(",
        ")",
        "((((((((((((((((((((((((((((",
        "))))))))))",
        "\"",
        "'",
        "\\",
        "url(",
        "url(\"",
        "url('a",
        "calc(",
        "calc(min(",
        "calc(min(1px, 2px)",
        "linear-gradient(",
        "linear-gradient(,,,,)",
        "linear-gradient(red,,blue)",
        "radial-gradient(circle,",
        ",,,,,,,,,,,,,,,,,,,,,,,",
        "1px,,,2px",
        "inset",
        "inset inset inset",
        "#",
        "#ffff",
        "#gg0000",
        "rgb(",
        "rgb(1,2)",
        "rgba(0,0,0,)",
        "oklch(0 0 0",
        "50%%",
        "%",
        "-",
        "+",
        ".",
        "1e999",
        "1e-999px",
        "99999999999999999999999999px",
        "-99999999999999999999999999px",
        "NaN",
        "inf",
        "\u{0}",
        "\u{202e}red\u{202c}",
        "\u{feff}",
        "1\u{0}px",
        "\u{1f600}",
        "é",
        "calc(1px * 2px)",
        "calc(min(1px * , 2px) + 1px)",
        "calc(min(1px,2px)+)",
        "calc(min(,))",
        "calc(min(1px * 1px * 1px, 2px))",
        "url(data:image/svg+xml;base64,AAAA)",
        "url(javascript:alert(1))",
        "to to to",
        "to top top",
        "0deg 0deg",
        "circle at at at",
        "circle circle circle",
    ];
    let props = [
        "background-color",
        "background-image",
        "background-size",
        "background-position",
        "background-repeat",
        "border-top-width",
        "border-top-style",
        "border-top-color",
        "border-right-width",
        "border-left-style",
        "border-bottom-color",
        "border-top-left-radius",
        "border-bottom-right-radius",
        "box-shadow",
        "opacity",
        "filter",
        "cursor",
        "outline",
        "border-block",
    ];
    let ctx = Ctx {
        width: Some(100.0),
        height: Some(50.0),
        has_children: false,
    };
    for prop in props {
        for v in nasty {
            for ctx in [Ctx::default(), ctx] {
                bounded(&[(prop, v)], &[], &ctx);
                // The same text on the `::after` box and in the other slots of a border.
                bounded(
                    &[("border-radius", "8px")],
                    &[
                        ("content", "\"\""),
                        ("position", "absolute"),
                        ("top", v),
                        (prop, v),
                        ("border-top-style", "solid"),
                    ],
                    &ctx,
                );
            }
        }
    }
}

#[test]
fn a_value_of_a_megabyte_is_read_in_work_that_grows_with_its_length() {
    let work = |len: usize| -> usize {
        let mut total = 0;
        let big = |piece: &str| piece.repeat(len / piece.len().max(1));
        for (name, piece) in [
            ("background-image", "linear-gradient(red, blue), "),
            ("background-image", "url(a.png), "),
            ("box-shadow", "1px 2px 3px red, "),
            ("box-shadow", "inset 1px 2px 3px red "),
            ("background-color", "red "),
            ("border-top-width", "1px "),
            ("border-top-left-radius", "1px "),
            ("opacity", "1 "),
        ] {
            let v = big(piece);
            let s = style_of(&[(name, &v), ("border-top-style", "solid")]);
            let (_, r) = paint_of(&s, &Ctx::default());
            total += r.work;
        }
        total
    };
    let (small, large) = (work(16 * 1024), work(1024 * 1024));
    // 64 times the text is at most 64 times the steps, with room for the fixed part.
    assert!(
        large <= small * 64 + 64,
        "{small} steps for 16 KiB, {large} for 1 MiB"
    );
    // And never more than a step for every eight bytes of text: nothing is looked at twice over.
    assert!(large <= 8 * 1024 * 1024 / 8, "{large}");
}

#[test]
fn a_style_with_thousands_of_properties_on_the_box_is_bounded_by_the_report() {
    let names: Vec<String> = (0..5000).map(|i| format!("filter-{i}")).collect();
    let mut after: Vec<(&str, &str)> = vec![
        ("content", "\"\""),
        ("position", "absolute"),
        ("border-top-style", "solid"),
    ];
    after.extend(names.iter().map(|n| (n.as_str(), "x")));
    let s = style_with_after(&[("border-radius", "8px")], &after);
    let (_, r) = paint_of(&s, &Ctx::default());
    assert!(r.entries.len() <= MAX_ENTRIES);
    assert!(r.omitted > 0);
}
