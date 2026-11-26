//! The CSS reader on well-formed and slightly loose text: rules, the simple selectors and their specificity, the
//! escapes of the export's `[data-node="..."]`, declarations, and what is refused and listed.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod css_common;
use css_common::*;
use html_import::css::{EntryKind, Limits, read};

#[test]
fn a_rule_is_its_selectors_and_its_declarations_in_the_order_written() {
    assert_eq!(
        lines(".a, #b>p { color : red ; margin:0 auto }\nspan{top:1px}"),
        [".a, #b>p{color:red; margin:0 auto}", "span{top:1px}"]
    );
}

#[test]
fn the_exports_own_css_is_read() {
    let css = "*,::before,::after{box-sizing:border-box}body{margin:0;font-family:'Inter Variable'}p{margin:0}\n\
        :root{--c-brand:#3366ff}:root[data-mode-c=\"dark\"]{--c-brand:#fff}\n\
        [data-node=\"n1\"]{position:absolute;left:0px}\n\
        [data-node=\"n1\"]::after{content:\"\";position:absolute;pointer-events:none;inset:-1px;border:2px solid #000}\n\
        @font-face{font-family:'Inter Variable';src:url('inter.woff2') format('woff2');font-weight:100 900}";
    let (s, r) = sheet(css);
    assert_eq!(
        dump(&s),
        [
            "*, *::before, *::after{box-sizing:border-box}",
            "body{margin:0; font-family:'Inter Variable'}",
            "p{margin:0}",
            ":root{--c-brand:#3366ff}",
            ":root[data-mode-c=dark]{--c-brand:#fff}",
            "[data-node=n1]{position:absolute; left:0px}",
            "[data-node=n1]::after{content:\"\"; position:absolute; pointer-events:none; inset:-1px; border:2px solid #000}",
        ]
    );
    // Only the font face is left out, and it is listed.
    assert_eq!(count(&r, "at_rule_skipped"), 1);
    assert_eq!(r.entries.len(), 1);
}

#[test]
fn a_selector_that_is_not_simple_is_refused_with_its_reason_and_the_others_of_the_list_stay() {
    let (s, r) = sheet(
        "a:hover, b, a:not(.x), c::placeholder, d + e, f ~ g, [h^=i], [j=k l], :first-child, \
         [m=n i], a::after:hover, a::after b {x:1}",
    );
    assert_eq!(dump(&s), ["b{x:1}"]);
    for (code, n) in [
        ("selector_pseudo_class", 3),
        ("selector_pseudo_element", 1),
        ("selector_sibling", 2),
        ("selector_attribute", 3),
        ("selector_invalid", 2),
    ] {
        assert_eq!(count(&r, code), n, "{code}");
    }
    assert!(r.entries.iter().all(|e| e.kind == EntryKind::Refused));
    assert_eq!(names(&r, "selector_sibling"), ["d + e", "f ~ g"]);
    // A rule with nothing readable is skipped whole, block and all.
    assert_eq!(lines("a:hover{x:1} b{y:2}"), ["b{y:2}"]);
}

#[test]
fn a_list_with_an_empty_selector_refuses_that_one() {
    let (s, r) = sheet("a,,b,{x:1}");
    assert_eq!(dump(&s), ["a, b{x:1}"]);
    assert_eq!(count(&r, "selector_invalid"), 2);
}

#[test]
fn at_rules_are_skipped_whole_and_listed_and_the_rules_around_them_stay() {
    let (s, r) = sheet(
        "a{x:1} @media (min-width:1px){ b{x:2} @supports (y:z){ c{x:3} } } d{x:4}\n\
         @import url(other.css); @charset \"utf-8\"; @keyframes k { from{x:1} to{x:2} } @container (x){ e{x:5} }\n\
         @property --p { syntax:'*'; inherits:false } f{x:6}",
    );
    assert_eq!(dump(&s), ["a{x:1}", "d{x:4}", "f{x:6}"]);
    assert_eq!(
        names(&r, "at_rule_skipped"),
        [
            "@media (min-width:1px)",
            "@charset \"utf-8\"",
            "@keyframes k",
            "@container (x)",
            "@property --p"
        ]
    );
    assert_eq!(names(&r, "import_skipped"), ["@import url(other.css)"]);
}

#[test]
fn layers_are_transparent() {
    let (s, r) = sheet(
        "@layer base, theme; @layer base { a{x:1} @layer inner { b{x:2} } } @layer { c{x:3} } d{x:4}",
    );
    assert_eq!(dump(&s), ["a{x:1}", "b{x:2}", "c{x:3}", "d{x:4}"]);
    assert!(r.entries.is_empty());
}

#[test]
fn a_rule_inside_a_rule_is_skipped_and_the_declarations_around_it_stay() {
    let (s, r) = sheet("a{x:1; .b{y:2} z:3; &:hover{w:4}}");
    assert_eq!(dump(&s), ["a{x:1; z:3}"]);
    assert_eq!(names(&r, "nested_rule_skipped"), [".b", "&:hover"]);
}

#[test]
fn strings_comments_and_brackets_hide_the_characters_that_end_a_piece() {
    assert_eq!(
        lines("a{content:\"}{;\";b:url(x;y}z);c:'\\''} b{d:e}/*{*/ c/*}*/{f:g}"),
        [
            "a{content:\"}{;\"; b:url(x;y}z); c:'\\''}",
            "b{d:e}",
            "c{f:g}"
        ]
    );
    // A string ends at its line.
    assert_eq!(lines("a{b:\"x\n;c:d}e{f:g}"), ["a{c:d}", "e{f:g}"]);
    // A backslash outside a string takes the next character with it.
    assert_eq!(lines(r"a{b:c\;d;e:f}"), [r"a{b:c\;d; e:f}"]);
}

#[test]
fn text_that_ends_inside_something_keeps_what_came_before_and_says_so() {
    for css in ["a{b:c", "a{b:c;d", "a{b:\"c", "a{b:url(c", "a{b:c/* x"] {
        let (s, r) = sheet(css);
        assert_eq!(s.rules.len(), 1, "{css}");
        assert!(count(&r, "unterminated") >= 1, "{css}");
    }
    assert_eq!(lines("a{b:c} d"), ["a{b:c}"]);
    let (s, r) = sheet("@media{a{b:c}");
    assert!(s.rules.is_empty());
    assert_eq!(count(&r, "unterminated"), 1);
    // A close brace with nothing open is counted and ignored.
    let (s, r) = sheet("} a{b:c} }}");
    assert_eq!(dump(&s), ["a{b:c}"]);
    assert_eq!(count(&r, "stray_close_brace"), 3);
}

#[test]
fn several_texts_are_one_sheet_and_the_byte_of_an_entry_counts_them_in_turn() {
    let mut r = html_import::css::Report::default();
    let texts = ["a{x:1} b:hover{x:2}", "c{x:3}", "d:hover{x:4}"];
    let s = read(texts, &Limits::default(), &mut r);
    assert_eq!(dump(&s), ["a{x:1}", "c{x:3}"]);
    let at: Vec<usize> = r.entries.iter().map(|e| e.at).collect();
    assert_eq!(at, [7, 25]);
    assert_eq!((s.rules[0].at, s.rules[1].at), (0, 19));
}

#[test]
fn a_rule_with_no_declaration_makes_no_rule() {
    assert!(lines("a{} b{;;} c{/* x */}").is_empty());
}

#[test]
fn white_space_before_and_after_does_not_count_against_the_caps() {
    // 5,000 spaces before a selector are not a selector too long, and the rule is read.
    let (s, r) = sheet(&format!("{}a{{x:1}}", " ".repeat(5_000)));
    assert_eq!(dump(&s), ["a{x:1}"]);
    assert!(r.entries.is_empty());
    // 1,300 spaces before a declaration, after it, and as comments, are not a value too long.
    let pad = " ".repeat(1_300);
    let (s, r) = sheet(&format!("a{{{pad}x:1{pad};{pad}y:2{pad}}}"));
    assert_eq!(dump(&s), ["a{x:1; y:2}"]);
    assert!(r.entries.is_empty());
    let (s, r) = sheet(&format!("{}a{{x:1}}{0}b{{y:2}}", "/**/".repeat(3_000)));
    assert_eq!(dump(&s), ["a{x:1}", "b{y:2}"]);
    assert!(r.entries.is_empty());
    // A long run inside a value still reads as white space and still counts as one value with its words.
    let (s, _) = sheet(&format!("a{{x:1{}2}}", " ".repeat(5_000)));
    assert_eq!(dump(&s), ["a{x:1 2}"]);
    // What is over the cap after the trim is still too long, and says so.
    let (s, r) = sheet(&format!("{}{{x:1}}b{{y:2}}", "a".repeat(5_000)));
    assert_eq!(dump(&s), ["b{y:2}"]);
    assert_eq!(count(&r, "selector_too_long"), 1);
}

#[test]
fn a_comment_in_a_selector_is_nothing_and_in_a_value_is_a_space() {
    assert_eq!(lines(".a/**/.b{x:1}"), [".a.b{x:1}"]);
    assert_eq!(lines("a /**/ b{x:1}"), ["a b{x:1}"]);
    assert_eq!(lines("a>/**/b{x:1}"), ["a>b{x:1}"]);
    assert_eq!(lines("[a/**/=/**/b]{x:1}"), ["[a=b]{x:1}"]);
    // Two names with a comment between are two names in a row, which is no selector.
    let (s, r) = sheet("a/**/b{x:1} c{y:2}");
    assert_eq!(dump(&s), ["c{y:2}"]);
    assert_eq!(count(&r, "selector_invalid"), 1);
    assert_eq!(
        lines("a{padding:1px/**/2px;margin:0/*x*/}"),
        ["a{padding:1px 2px; margin:0}"]
    );
    // A NUL in the text is the replacement character, as CSS says; it is not a comment.
    assert_eq!(lines("a{x:\0}"), ["a{x:\u{fffd}}"]);
    assert_eq!(lines(".a\0{x:1}"), [".a\u{fffd}{x:1}"]);
}

#[test]
fn the_html_comment_marks_at_the_top_are_ignored() {
    assert_eq!(lines("<!-- a{x:1} --> b{y:2}"), ["a{x:1}", "b{y:2}"]);
    assert_eq!(
        lines("<!--\na{x:1}\n-->\n<!-- -->b{y:2}"),
        ["a{x:1}", "b{y:2}"]
    );
    // Inside a block they are not marks: a declaration that is not one.
    let (s, r) = sheet("a{<!-- x:1 -->; y:2}");
    assert_eq!(dump(&s), ["a{y:2}"]);
    assert_eq!(count(&r, "invalid_declaration"), 1);
}

#[test]
fn a_layer_cut_off_by_the_end_of_the_text_is_unterminated() {
    let (s, r) = sheet("@layer a{b{x:1}");
    assert_eq!(dump(&s), ["b{x:1}"]);
    assert_eq!(count(&r, "unterminated"), 1);
    let (s, r) = sheet("@layer a{@layer b{c{x:1}}");
    assert_eq!(dump(&s), ["c{x:1}"]);
    assert_eq!(count(&r, "unterminated"), 1);
    let (_, r) = sheet("@layer a{b{x:1}}");
    assert_eq!(count(&r, "unterminated"), 0);
}

#[test]
fn text_before_a_stray_close_brace_is_listed_and_a_bare_brace_is_only_counted() {
    let (s, r) = sheet("junk } a{x:1} } more {}");
    assert_eq!(dump(&s), ["a{x:1}"]);
    assert_eq!(names(&r, "invalid_statement"), ["junk"]);
    assert_eq!(count(&r, "stray_close_brace"), 2);
}

#[test]
fn text_that_ends_after_a_rule_or_inside_a_comment_says_so() {
    for css in [
        "a{x:1} d",
        "a{x:1} /* c",
        "a{x:1} \"s",
        "a{x:1} (",
        "a{x:1} @media",
    ] {
        assert_eq!(count(&sheet(css).1, "unterminated"), 1, "{css}");
    }
    assert_eq!(count(&sheet("a{x:1} d;").1, "unterminated"), 0);
}

#[test]
fn at_names_are_folded_to_lower_case() {
    let (s, r) = sheet("@LAYER x{a{y:1}} @Import \"x\"; @MEDIA print{b{y:2}} @LAYER a,b; c{y:3}");
    assert_eq!(dump(&s), ["a{y:1}", "c{y:3}"]);
    assert_eq!(count(&r, "import_skipped"), 1);
    assert_eq!(names(&r, "at_rule_skipped"), ["@MEDIA print"]);
}

#[test]
fn a_string_ends_at_any_line_end_and_a_url_keeps_its_brackets() {
    for end in ["\n", "\r", "\u{c}"] {
        let (s, r) = sheet(&format!("a{{b:\"x{end};c:d}} e{{f:g}}"));
        assert_eq!(dump(&s), ["a{c:d}", "e{f:g}"], "{end:?}");
        assert_eq!(count(&r, "invalid_declaration"), 1);
    }
    assert_eq!(lines("a{b:url(a(b)c;d);e:f}"), ["a{b:url(a(b)c;d); e:f}"]);
    let (s, r) = sheet("a[x=\"1\n]{y:1} b{z:2}");
    assert_eq!(dump(&s), ["b{z:2}"]);
    assert_eq!(count(&r, "selector_invalid"), 1);
}

#[test]
fn twenty_layers_one_after_the_other_are_not_nested() {
    let css = "@layer x{a{y:1}}".repeat(20);
    let (s, r) = sheet(&css);
    assert_eq!(s.rules.len(), 20);
    assert!(r.entries.is_empty());
}

#[test]
fn an_invalid_selector_of_a_list_is_refused_alone() {
    // CSS would drop the whole rule; this keeps the rest of the list, as the module says.
    assert_eq!(lines("a, 1b, c{x:1}"), ["a, c{x:1}"]);
}

#[test]
fn selectors_are_counted_over_the_whole_import() {
    let l = Limits {
        max_selectors: 5,
        ..Limits::default()
    };
    let (s, r) = sheet_with("a,b,c{x:1} d,e,f{y:2} g{z:3}", &l);
    assert_eq!(dump(&s), ["a, b, c{x:1}", "d, e{y:2}"]);
    assert_eq!(count(&r, "selector_limit"), 1);
    assert_eq!(r.entries[0].kind, EntryKind::Cut);
    // `a,a,a,...` is a selector for each two bytes: twenty-five rules of a thousand are stopped at 10,000.
    let css = format!("{}{{x:1}}", "a,".repeat(1_000)).repeat(25);
    let (s, r) = sheet(&css);
    assert_eq!(
        s.rules.iter().map(|r| r.selectors.len()).sum::<usize>(),
        10_000
    );
    assert_eq!(s.rules.len(), 10);
    assert_eq!(count(&r, "selector_limit"), 1);
}

#[test]
fn only_ascii_white_space_is_trimmed_around_a_selector_a_statement_and_a_layer_name() {
    // A no-break space is a character of a name, not space: it is in the selector, in both places.
    assert_eq!(lines("\u{a0}b{x:1}"), ["\u{a0}b{x:1}"]);
    assert_eq!(lines("b\u{a0}{x:1}"), ["b\u{a0}{x:1}"]);
    assert_eq!(lines("a,\u{a0}c{x:1}"), ["a, \u{a0}c{x:1}"]);
    assert_eq!(lines("\t\n\u{c}\r b \r\n{x:1}"), ["b{x:1}"]);
    // A statement and an at-rule name have their own trim.
    let (_, r) = sheet("\u{a0}junk; @media\u{a0}x{a{b:c}}");
    assert_eq!(names(&r, "invalid_statement"), ["\u{a0}junk"]);
    assert_eq!(names(&r, "at_rule_skipped"), ["@media\u{a0}x"]);
    // And a declaration in a rule.
    assert_eq!(lines("a{b:\u{a0}c\u{a0}}"), ["a{b:\u{a0}c\u{a0}}"]);
}

#[test]
fn text_before_the_brace_that_closes_a_layer_is_listed_too() {
    for css in [
        "@layer a{ b{x:1} c:d }",
        "@layer a{ b{x:1} junk }",
        "@layer a{@layer b{ junk } b{x:1}}",
    ] {
        let (s, r) = sheet(css);
        assert_eq!(s.rules.len(), 1, "{css}");
        assert_eq!(r.entries.len(), 1, "{css}");
        assert_eq!(r.entries[0].code, "invalid_statement", "{css}");
    }
    assert_eq!(
        names(&sheet("@layer a{ b{x:1} c:d }").1, "invalid_statement"),
        ["c:d"]
    );
    // Nothing before it, and a rule before it, say nothing.
    assert!(sheet("@layer a{ b{x:1} }").1.entries.is_empty());
    assert!(sheet("@layer a{}").1.entries.is_empty());
    assert!(sheet("@layer a{ ; }").1.entries.is_empty());
}

#[test]
fn a_selector_with_a_string_that_ends_at_its_line_drops_the_whole_list_and_the_next_rule_is_read() {
    // The line end ends the string and the `{` after it opens the block: the rule is invalid as a whole, `a` too,
    // which is not what reading the list selector by selector would do.
    let (s, r) = sheet("a, [x=\"1\n]{c:d} b{e:f}");
    assert_eq!(dump(&s), ["b{e:f}"]);
    assert_eq!(names(&r, "selector_invalid").len(), 1);
    assert!(r.entries[0].message.contains("line"), "{:?}", r.entries[0]);
    // Without the line end the list is read selector by selector.
    assert_eq!(lines("a, [x=\"1\"]{c:d}"), ["a, [x=1]{c:d}"]);
}

#[test]
fn a_backslash_before_any_line_end_joins_the_lines_of_a_string_in_a_rule() {
    for end in ["\n", "\r", "\r\n", "\u{c}"] {
        assert_eq!(
            lines(&format!("a{{b:\"x\\{end}y\";c:d}}")),
            [format!("a{{b:\"x\\{end}y\"; c:d}}")],
            "{end:?}"
        );
        assert_eq!(
            lines(&format!("[a=\"x\\{end}y\"]{{c:d}}")),
            ["[a=xy]{c:d}"],
            "{end:?}"
        );
    }
}
