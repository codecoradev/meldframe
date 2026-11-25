//! The selector reader: the simple selectors of `docs/import-html.md` section 3.3, their specificity, the escapes of
//! the export's `[data-node="..."]`, and what is refused, with its code. A pure function of the text.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod css_common;
use css_common::*;
use html_import::css::selector::{MAX_COMPOUNDS, MAX_SIMPLES, parse, split_list};

/// The selector in `text`, as [`sel`] writes it.
fn read(text: &str) -> String {
    sel(&parse(text, &mut 0).unwrap_or_else(|e| panic!("{text:?} is refused: {e:?}")))
}

/// The code `text` is refused with.
fn refused(text: &str) -> &'static str {
    match parse(text, &mut 0) {
        Ok(s) => panic!("{text:?} is read as {}", sel(&s)),
        Err(e) => e.code,
    }
}

#[test]
fn every_simple_selector_is_read() {
    for (text, want) in [
        ("*", "*"),
        ("div", "div"),
        (".x", ".x"),
        ("#y", "#y"),
        ("div.x#y", "div.x#y"),
        ("*.x", ".x"),
        ("[d]", "[d]"),
        ("[d=e]", "[d=e]"),
        ("[ d = \"e f\" ]", "[d=e f]"),
        ("[d='e']", "[d=e]"),
        ("[D=E]", "[d=E]"),
        (":root", ":root"),
        ("html:root", "html:root"),
        ("a b", "a b"),
        ("a>b", "a>b"),
        ("a  >  b c", "a>b c"),
        ("  a\n\tb  ", "a b"),
        ("DIV.X", "div.X"),
    ] {
        assert_eq!(read(text), want, "{text}");
    }
}

#[test]
fn the_pseudo_elements_before_and_after_end_a_selector_in_both_spellings() {
    for (text, want) in [
        ("a::after", "a::after"),
        ("a:before", "a::before"),
        ("[data-node=n]::before", "[data-node=n]::before"),
        ("::after", "*::after"),
        ("a b::AFTER", "a b::after"),
    ] {
        assert_eq!(read(text), want, "{text}");
    }
    assert_eq!(refused("a::after b"), "selector_invalid");
    assert_eq!(refused("a::after.x"), "selector_invalid");
    assert_eq!(refused("a::after::before"), "selector_invalid");
}

#[test]
fn specificity_counts_ids_then_classes_attributes_and_root_then_types_and_the_pseudo_element() {
    let got: Vec<_> = [
        "*",
        "p",
        "p.x",
        "#i",
        "[a][b]",
        ":root",
        "ul li a",
        "a::after",
        "#i.x.y p>b",
    ]
    .iter()
    .map(|t| spec(&parse(t, &mut 0).unwrap()))
    .collect();
    assert_eq!(
        got,
        [
            (0, 0, 0),
            (0, 0, 1),
            (0, 1, 1),
            (1, 0, 0),
            (0, 2, 0),
            (0, 1, 0),
            (0, 0, 3),
            (0, 0, 2),
            (1, 2, 2)
        ]
    );
    // One number orders by the three in turn, and a count over 255 stays 255.
    let one = |t: &str| parse(t, &mut 0).unwrap().specificity;
    assert!(one("#a") > one(".a.b.c.d"));
    assert!(one(".a") > one("a b c d"));
    assert_eq!(one(&".a".repeat(16)) >> 8 & 255, 16);
}

#[test]
fn css_escapes_are_read_in_names_and_strings() {
    // The export writes a hex escape, with the space that ends it, for every character that is not a letter or a digit.
    for (text, want) in [
        (r#"[data-node="a\3c b\22 c"]"#, "[data-node=a<b\"c]"),
        (r".\31 a", ".1a"),
        (r"#a\:b", "#a:b"),
        (r#"[data-name="q\"r"]"#, "[data-name=q\"r]"),
        (r"[x=\41 b]", "[x=Ab]"),
        (r".\{\}", ".{}"),
        (r"\{\}", "{}"),
        (
            r#"[data-node="\1f600 \110000 \0 "]"#,
            "[data-node=\u{1f600}\u{fffd}\u{fffd}]",
        ),
        ("[a=\"x\\\ny\"]", "[a=xy]"),
        (".\u{e9}\u{1f600}", ".\u{e9}\u{1f600}"),
    ] {
        assert_eq!(read(text), want, "{text}");
    }
    assert_eq!(refused("[a=\"x"), "selector_invalid");
}

#[test]
fn what_is_not_simple_is_refused_with_its_code() {
    for (text, code) in [
        ("a:hover", "selector_pseudo_class"),
        ("a:not(.x)", "selector_pseudo_class"),
        (":first-child", "selector_pseudo_class"),
        (":nth-child(2n+1)", "selector_pseudo_class"),
        ("c::placeholder", "selector_pseudo_element"),
        ("c::selection", "selector_pseudo_element"),
        ("d + e", "selector_sibling"),
        ("f ~ g", "selector_sibling"),
        ("f+g", "selector_sibling"),
        ("[h^=i]", "selector_attribute"),
        ("[h$=i]", "selector_attribute"),
        ("[h*=i]", "selector_attribute"),
        ("[h~=i]", "selector_attribute"),
        ("[h|=i]", "selector_attribute"),
        ("[j=k l]", "selector_attribute"),
        ("[m=n i]", "selector_attribute"),
        ("", "selector_invalid"),
        (" ", "selector_invalid"),
        (">a", "selector_invalid"),
        ("a>", "selector_invalid"),
        ("a>>b", "selector_invalid"),
        (".", "selector_invalid"),
        ("#", "selector_invalid"),
        ("[]", "selector_invalid"),
        ("[a", "selector_invalid"),
        ("a*", "selector_invalid"),
        ("a)", "selector_invalid"),
        ("a{", "selector_invalid"),
        ("&", "selector_invalid"),
    ] {
        assert_eq!(refused(text), code, "{text:?}");
    }
}

#[test]
fn a_selector_is_bounded_in_compounds_and_in_parts() {
    assert!(parse(&vec!["a"; MAX_COMPOUNDS].join(" "), &mut 0).is_ok());
    assert_eq!(
        refused(&vec!["a"; MAX_COMPOUNDS + 1].join(" ")),
        "selector_too_complex"
    );
    assert!(parse(&format!("a{}", ".b".repeat(MAX_SIMPLES - 1)), &mut 0).is_ok());
    assert_eq!(
        refused(&format!("a{}", ".b".repeat(MAX_SIMPLES))),
        "selector_too_complex"
    );
}

#[test]
fn a_list_is_cut_at_the_commas_outside_strings_brackets_and_escapes() {
    assert_eq!(split_list("a, b ,c"), ["a", " b ", "c"]);
    assert_eq!(split_list("a"), ["a"]);
    assert_eq!(split_list(""), [""]);
    assert_eq!(split_list(",a,"), ["", "a", ""]);
    assert_eq!(
        split_list(r#"[a="x,y"], [b='\',']  ,c\,d, :not(e,f)"#),
        [r#"[a="x,y"]"#, r#" [b='\',']  "#, r"c\,d", " :not(e,f)"]
    );
    assert_eq!(split_list("[a=,"), ["[a=,"]);
}

#[test]
fn hostile_selectors_never_panic_and_cost_work_in_line_with_their_length() {
    let doc = "[data-node=\"a\\3c b\"]>.c #d::after, a:hover, \\, \\\u{e9}\u{1f600} [x='\\";
    for i in 0..=doc.len() {
        if doc.is_char_boundary(i) {
            let _ = parse(&doc[..i], &mut 0);
            split_list(&doc[..i]);
        }
    }
    for text in [
        "a ".repeat(100_000),
        ".a".repeat(100_000),
        "[".repeat(100_000),
        "\\".repeat(100_000),
        "\\1".repeat(100_000),
        ":".repeat(100_000),
        "a>".repeat(100_000),
        "\u{1f600}".repeat(100_000),
    ] {
        let mut work = 0;
        let _ = parse(&text, &mut work);
        assert!(work <= 2 * text.len() + 16, "{work} for {}", text.len());
        assert!(split_list(&text).len() <= text.len() + 1);
    }
}
