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
fn declarations_fold_property_names_and_read_important() {
    let (d, r) = decls(
        "COLOR : Red ! IMPORTANT; margin:0!important;--Brand: #FFF ;--empty:;padding:1px /* c */ 2px;\
         background:url(a;b) , none;content:\"a;b}c\"; font-family: 'A\\';B'",
    );
    assert_eq!(
        d,
        [
            "color:Red!",
            "margin:0!",
            "--Brand:#FFF",
            "--empty:",
            "padding:1px   2px",
            "background:url(a;b) , none",
            "content:\"a;b}c\"",
            "font-family:'A\\';B'"
        ]
    );
    assert!(r.entries.is_empty());
}

#[test]
fn a_piece_that_is_not_a_declaration_is_listed_and_the_rest_stay() {
    let (d, r) = decls("color red; :x; 1a:b; a b:c; width:; height:1px; }; x:y");
    assert_eq!(d, ["height:1px", "x:y"]);
    assert_eq!(
        names(&r, "invalid_declaration"),
        ["color red", ":x", "1a:b", "a b:c"]
    );
    assert_eq!(names(&r, "empty_value"), ["width:"]);
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
    assert_eq!(at, [6, 25]);
    assert_eq!((s.rules[0].at, s.rules[1].at), (0, 19));
}

#[test]
fn a_rule_with_no_declaration_makes_no_rule() {
    assert!(lines("a{} b{;;} c{/* x */}").is_empty());
}
