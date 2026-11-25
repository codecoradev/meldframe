//! The selector reader: the simple selectors of `docs/import-html.md` section 3.3, their specificity, the escapes of
//! the export's `[data-node="..."]`, and what is refused, with its code. A pure function of the text.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use html_import::css::selector::{MAX_COMPOUNDS, MAX_SIMPLES, parse, split_list};
use html_import::css::{Combinator, Pseudo, Selector, Simple};

/// A selector as text: `div>.a #b[x=y]::after`. Specificity is written by [`spec`].
pub fn sel(s: &Selector) -> String {
    let mut out = String::new();
    for (i, c) in s.compounds.iter().enumerate() {
        if i > 0 {
            out.push(match s.combinators[i - 1] {
                Combinator::Descendant => ' ',
                Combinator::Child => '>',
            });
        }
        if c.simples.is_empty() {
            out.push('*');
        }
        for simple in &c.simples {
            match simple {
                Simple::Tag(t) => out.push_str(t),
                Simple::Class(c) => out.push_str(&format!(".{c}")),
                Simple::Id(i) => out.push_str(&format!("#{i}")),
                Simple::Attr(n, None) => out.push_str(&format!("[{n}]")),
                Simple::Attr(n, Some(v)) => out.push_str(&format!("[{n}={v}]")),
                Simple::Root => out.push_str(":root"),
            }
        }
    }
    match s.pseudo {
        Some(Pseudo::Before) => out.push_str("::before"),
        Some(Pseudo::After) => out.push_str("::after"),
        None => {}
    }
    out
}

/// The specificity as `(ids, classes, types)`.
pub fn spec(s: &Selector) -> (u32, u32, u32) {
    (
        s.specificity >> 16,
        (s.specificity >> 8) & 255,
        s.specificity & 255,
    )
}

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
fn names_keep_their_case_and_ascii_space_only_is_trimmed() {
    assert_eq!(read("#Y.Z"), "#Y.Z");
    assert_eq!(read("\t a \u{c}"), "a");
    // A no-break space is a character of the name, not space.
    assert_eq!(read("a\u{a0}b"), "a\u{a0}b");
    // Six hex digits at most go to an escape (0x123456 is no character); the seventh is a letter of the name.
    assert_eq!(read(".\\1234567"), ".\u{fffd}7");
}

#[test]
fn a_name_must_be_a_name() {
    for text in [
        ".1a",
        "#1a",
        "1a",
        ".-1a",
        "#-1",
        ".-",
        "[1a=b]",
        "[-1=b]",
        "[a=]",
        "[a=\"x\n\"]",
        "\\",
        ".a\\\nb",
        "a\\",
        "[a=\"x\r\"]",
        "::root:x",
    ] {
        let code = refused(text);
        assert!(code.starts_with("selector_"), "{text:?}");
    }
    // An escape can start a name with a digit, a double hyphen can start one, and a quoted value may be empty.
    assert_eq!(read(".\\31 a"), ".1a");
    assert_eq!(read(".--x"), ".--x");
    assert_eq!(read(".-a"), ".-a");
    assert_eq!(read("[a=\"\"]"), "[a=]");
    assert_eq!(refused("::root"), "selector_pseudo_element");
    assert_eq!(refused("a:root:hover"), "selector_pseudo_class");
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

#[test]
fn a_backslash_before_any_line_end_is_a_line_continuation_in_a_string_and_no_escape_outside() {
    for end in ["\n", "\r", "\r\n", "\u{c}"] {
        assert_eq!(read(&format!("[a=\"x\\{end}y\"]")), "[a=xy]", "{end:?}");
        // Outside a string the backslash and the line end are not an escape: the selector is invalid.
        assert_eq!(
            refused(&format!(".a\\{end}b")),
            "selector_invalid",
            "{end:?}"
        );
    }
    // A line end that is not escaped ends the string, which makes the selector invalid.
    for end in ["\n", "\r", "\r\n", "\u{c}"] {
        assert_eq!(
            refused(&format!("[a=\"x{end}y\"]")),
            "selector_invalid",
            "{end:?}"
        );
    }
}

#[test]
fn only_ascii_white_space_is_trimmed_around_a_selector() {
    for (text, want) in [
        ("\u{a0}b", "\u{a0}b"),
        ("b\u{a0}", "b\u{a0}"),
        ("\u{a0}b\u{a0}", "\u{a0}b\u{a0}"),
        ("\u{2003}b", "\u{2003}b"),
    ] {
        assert_eq!(read(text), want, "{text:?}");
    }
    assert_eq!(read("\t\n\u{c}\rb \r\n"), "b");
}

#[test]
fn the_work_of_a_selector_is_one_step_for_each_character_it_read() {
    for text in [
        "a",
        "div.a#b[c=d] > e .f::after",
        "[a=\"x y\"]",
        ".\\31 a",
        "\u{e9}.\u{1f600}",
        "a, b".split(',').next().unwrap(),
    ] {
        let mut work = 0;
        let _ = parse(text, &mut work);
        assert_eq!(work, text.chars().count(), "{text:?}");
    }
    // An invalid one costs what it read before it stopped, and nothing is free.
    let mut work = 0;
    let _ = parse("a:hover b", &mut work);
    assert!(work >= 7, "{work}");
}

#[test]
fn a_count_of_ids_or_classes_over_255_stays_255_in_its_byte() {
    let compound = |part: &str| part.repeat(MAX_SIMPLES);
    let many = |part: &str| vec![compound(part); MAX_COMPOUNDS].join(" ");
    assert_eq!(spec(&parse(&many(".a"), &mut 0).unwrap()), (0, 255, 0));
    assert_eq!(spec(&parse(&many("#a"), &mut 0).unwrap()), (255, 0, 0));
    // 255 is not clamped away: fifteen compounds of sixteen and one of fifteen is 255 exactly.
    let mut parts = vec![compound(".a"); MAX_COMPOUNDS - 1];
    parts.push(".a".repeat(MAX_SIMPLES - 1));
    assert_eq!(spec(&parse(&parts.join(" "), &mut 0).unwrap()), (0, 255, 0));
    parts.pop();
    parts.push(".a".repeat(MAX_SIMPLES - 2));
    assert_eq!(spec(&parse(&parts.join(" "), &mut 0).unwrap()), (0, 254, 0));
}
