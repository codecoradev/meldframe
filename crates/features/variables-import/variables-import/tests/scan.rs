//! The scanner: which custom properties it keeps and for which mode, what it leaves out, and that hostile text is
//! read in bounds. It reads text; it opens nothing and runs nothing.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use variables_import::scan::{Scan, Section, scan};

/// `(name, value, section, light, dark)` of every kept declaration.
fn kept(s: &Scan) -> Vec<(&str, &str, Section, bool, bool)> {
    s.decls
        .iter()
        .map(|d| {
            (
                d.name.as_str(),
                d.value.as_str(),
                d.section,
                d.light,
                d.dark,
            )
        })
        .collect()
}

fn codes(s: &Scan) -> Vec<&'static str> {
    s.left.iter().map(|l| l.code).collect()
}

#[test]
fn root_and_theme_declarations_are_kept_with_their_section_and_mode() {
    let s = scan(
        ":root { --a: #111; color: red; } @theme { --color-b: #222 } @theme inline { --color-c: var(--a) }",
    );
    assert_eq!(
        kept(&s),
        [
            ("--a", "#111", Section::Root, true, false),
            ("--color-b", "#222", Section::Theme, true, false),
            ("--color-c", "var(--a)", Section::Theme, true, false),
        ]
    );
    assert_eq!(s.other_properties, 1);
}

#[test]
fn the_selectors_and_queries_that_pick_the_dark_mode() {
    for rule in [
        ".dark { --a: 1 }",
        ":root.dark { --a: 1 }",
        "html.dark { --a: 1 }",
        ".dark * { --a: 1 }",
        "[data-theme=dark] { --a: 1 }",
        "[data-theme=\"dark\"] { --a: 1 }",
        ":root[data-mode='dark'] { --a: 1 }",
        "@media (prefers-color-scheme: dark) { :root { --a: 1 } }",
        "@media (prefers-color-scheme:dark){:root{--a:1}}",
        "@layer base { .dark { --a: 1 } }",
    ] {
        let s = scan(rule);
        assert_eq!(
            kept(&s),
            [("--a", "1", Section::Root, false, true)],
            "{rule}"
        );
    }
}

#[test]
fn light_selectors_are_the_default_mode_and_a_list_sets_both_modes() {
    for rule in [
        ":root",
        "html",
        ":host",
        "body",
        ".light",
        "[data-theme=light]",
        ":root:not(.dark)",
        "@media (prefers-color-scheme: light) { :root",
    ] {
        let close = if rule.starts_with('@') {
            " { --a: 1 } }"
        } else {
            " { --a: 1 }"
        };
        let s = scan(&format!("{rule}{close}"));
        assert_eq!(
            kept(&s),
            [("--a", "1", Section::Root, true, false)],
            "{rule}"
        );
    }
    let s = scan(":root, .dark { --a: 1 }");
    assert_eq!(kept(&s), [("--a", "1", Section::Root, true, true)]);
}

#[test]
fn a_rule_that_is_not_the_root_is_left_out_by_its_selector() {
    for rule in [
        ".card",
        "button",
        ".dark .card",
        ":root .x",
        ":root, .card",
        "[data-theme=blue]",
        "#a",
    ] {
        let s = scan(&format!("{rule} {{ --a: 1 }}"));
        assert!(s.decls.is_empty(), "{rule}");
        assert_eq!(codes(&s), ["selector_skipped"], "{rule}");
        assert_eq!(s.left[0].what, rule);
    }
}

#[test]
fn imports_are_never_followed_and_other_at_rules_are_left_out_whole() {
    let s = scan(
        "@charset \"utf-8\"; @import \"tailwindcss\"; @import url(x.css) layer(base); @plugin \"p\"; @layer a, b; \
         @keyframes k { from { --a: 1 } } @font-face { --b: 2 } @container (min-width: 1px) { :root { --c: 3 } } \
         @supports (color: red) { :root { --d: 4 } } @media print { :root { --e: 5 } } @theme { --color-ok: #fff }",
    );
    assert_eq!(
        kept(&s),
        [("--color-ok", "#fff", Section::Theme, true, false)]
    );
    assert_eq!(
        codes(&s),
        [
            "at_rule_skipped",
            "import_skipped",
            "import_skipped",
            "at_rule_skipped",
            "at_rule_skipped",
            "at_rule_skipped",
            "at_rule_skipped",
            "at_rule_skipped",
            "at_rule_skipped",
        ]
    );
}

#[test]
fn comments_strings_and_important_do_not_change_what_is_read() {
    let s = scan(
        "/* :root { --no: 1 } */ :root { /* --no2: 2; */ --a: \"x;}\" ; --b: url(\"a;b{c}\") !important; --c: 3 /* tail */; }",
    );
    assert_eq!(
        kept(&s),
        [
            ("--a", "\"x;}\"", Section::Root, true, false),
            ("--b", "url(\"a;b{c}\")", Section::Root, true, false),
            ("--c", "3", Section::Root, true, false),
        ]
    );
}

#[test]
fn text_that_ends_inside_something_is_marked_and_what_came_before_is_kept() {
    for cut in [
        ":root { --a: 1; --b: \"x",
        ":root { --a: 1; /* x",
        ":root { --a: 1; --b: calc(",
        ":root { --a: 1; --b",
    ] {
        let s = scan(cut);
        assert_eq!(s.decls[0].name, "--a", "{cut}");
        assert!(s.unterminated || s.decls.len() == 1, "{cut}");
    }
}

#[test]
fn broken_text_never_panics() {
    for t in [
        "",
        "{",
        "}",
        "}}}}",
        ";;;;",
        "@",
        "@{",
        ":root {",
        ":root { --a",
        ":root { --a:",
        ":root { --a: url(",
        "/*",
        "@media { { { {",
        ":root { --a: #fff; } }",
        "--a: #fff;",
        ":root { --a: '\\",
        ":root { --a: \"\\\u{e9}",
        "\u{e9}\u{e9} { --a: 1 }",
        ":root { --\u{e9}: \u{1f600}; }",
        ":root{--a:#fff}.dark{--a:}",
    ] {
        let _ = scan(t);
    }
    let text = include_str!("fixtures/representative-theme.css");
    for end in (0..text.len())
        .filter(|e| text.is_char_boundary(*e))
        .step_by(5)
    {
        let _ = scan(&text[..end]);
    }
}

#[test]
fn nesting_is_bounded_and_a_million_blocks_do_not_grow_the_stack() {
    let layers = format!(
        "{}:root {{ --a: 1 }}{}",
        "@layer a {".repeat(40),
        "}".repeat(40)
    );
    let s = scan(&layers);
    assert!(s.decls.is_empty());
    assert_eq!(codes(&s), ["too_deep"]);
    for open in ["@media print {", "@layer a {", ".x {"] {
        let n = 1_000_000;
        let deep = format!("{}:root {{ --a: 1 }}{}", open.repeat(n), "}".repeat(n));
        assert!(scan(&deep).decls.is_empty(), "{open}");
    }
}

#[test]
fn a_long_selector_list_and_more_declarations_than_the_limit_are_bounded() {
    let sel = vec![".x"; 20_000].join(",");
    assert!(scan(&format!("{sel} {{ --a: 1 }}")).decls.is_empty());
    let mut t = String::from(":root {");
    for k in 0..20_100 {
        t.push_str(&format!("--v{k}:1;"));
    }
    t.push('}');
    let s = scan(&t);
    assert_eq!(s.decls.len(), variables_import::scan::MAX_DECLS);
    assert!(s.too_many);
}

/// The bytes the structures of a scan keep: names, values and the sources they came from.
fn retained(s: &Scan) -> usize {
    let decls: usize = s.decls.iter().map(|d| d.name.len() + d.value.len()).sum();
    let sources: usize = s.sources.iter().map(String::len).sum();
    let left: usize = s.left.iter().map(|l| l.what.len() + l.message.len()).sum();
    decls + sources + left
}

#[test]
fn what_a_scan_keeps_is_bounded_by_its_input_not_by_its_selectors() {
    // One rule with a 96 KB selector list and 2,000 declarations: the selector is not copied for each of them.
    let mut text = ":root,".repeat(16_000) + ":root {";
    for i in 0..2_000 {
        text.push_str(&format!("--a{i}:1;"));
    }
    text.push('}');
    let s = scan(&text);
    assert_eq!(s.decls.len(), 2_000);
    assert!(retained(&s) < 100_000, "{} bytes kept", retained(&s));
    assert!(s.sources.iter().all(|x| x.len() <= 120));
    assert!(s.sources.len() <= 2, "one rule is one source");
}

#[test]
fn many_rules_keep_at_most_a_short_source_each_and_the_same_rule_once() {
    let mut text = String::new();
    for i in 0..3_000 {
        text.push_str(&format!(".dark {{ --a{i}: 1 }} .dark {{ --b{i}: 1 }}"));
    }
    let s = scan(&text);
    assert_eq!(s.decls.len(), 6_000);
    assert_eq!(s.sources, [".dark"], "the same source is kept once");
}

#[test]
fn only_a_plain_dark_or_light_query_picks_a_mode_and_other_forms_are_skipped_by_code() {
    for q in [
        "(prefers-color-scheme: dark)",
        "(prefers-color-scheme:dark)",
        "( prefers-color-scheme : dark )",
        "\t(prefers-color-scheme:\ndark)",
        "screen and (prefers-color-scheme: dark)",
        "all and (prefers-color-scheme: dark)",
        "(PREFERS-COLOR-SCHEME: DARK)",
    ] {
        let s = scan(&format!("@media {q} {{ :root {{ --a: 1 }} }}"));
        assert_eq!(
            kept(&s),
            [("--a", "1", Section::Root, false, true)],
            "{q:?}"
        );
    }
    let s = scan("@media(prefers-color-scheme:dark){:root{--a:1}}");
    assert_eq!(kept(&s), [("--a", "1", Section::Root, false, true)]);
    for q in [
        "not (prefers-color-scheme: dark)",
        "(prefers-color-scheme: dark) and (min-width: 600px)",
        "(min-width: 600px) and (prefers-color-scheme: dark)",
        "(prefers-color-scheme: dark), print",
        "(prefers-color-scheme: no-preference)",
        "not all and (prefers-color-scheme: dark)",
    ] {
        let s = scan(&format!("@media {q} {{ :root {{ --a: 1 }} }}"));
        assert!(s.decls.is_empty(), "{q}");
        assert_eq!(codes(&s), ["media_query_skipped"], "{q}");
    }
}

/// Every character that changes how text reads without showing, or hides a message in it: the format characters
/// (Unicode category Cf: soft hyphen, Arabic marks, the bidi controls, the word joiner and invisible operators, the
/// byte order mark, the annotation anchors, the musical and shorthand format controls), the variation selectors and the
/// tag block. The line and paragraph separators are white space and are collapsed instead.
fn invisible_chars() -> Vec<char> {
    let ranges: [(u32, u32); 19] = [
        (0xAD, 0xAD),
        (0x600, 0x605),
        (0x61C, 0x61C),
        (0x6DD, 0x6DD),
        (0x70F, 0x70F),
        (0x180E, 0x180E),
        (0x200B, 0x200F),
        (0x202A, 0x202E),
        (0x2060, 0x206F),
        (0xFE00, 0xFE0F),
        (0xFEFF, 0xFEFF),
        (0xFFF9, 0xFFFB),
        (0x110BD, 0x110BD),
        (0x1BCA0, 0x1BCA3),
        (0x1D173, 0x1D17A),
        (0xE0000, 0xE007F),
        (0xE0100, 0xE01EF),
        (0x13430, 0x13438),
        (0x1, 0x1F),
    ];
    ranges
        .iter()
        .flat_map(|&(a, b)| (a..=b).filter_map(char::from_u32))
        .collect()
}

#[test]
fn what_a_rule_is_called_in_the_scan_has_no_invisible_character_in_it() {
    // White space is collapsed instead (a tab, a no-break space), which the second check below covers.
    let list: Vec<char> = invisible_chars()
        .into_iter()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(list.len() > 300);
    for c in list {
        let s = scan(&format!(".card{c}x {{ --a: 1 }}"));
        assert_eq!(s.left[0].what, ".cardx", "U+{:04X}", c as u32);
    }
    let s = scan(".card\u{a0}\u{3000}x { --a: 1 }");
    assert_eq!(
        s.left[0].what, ".card x",
        "white space is collapsed, not dropped"
    );
}

#[test]
fn the_message_for_an_at_rule_without_a_block_is_clean_and_short() {
    // The name runs to the first white space, `(` or `"`, so it can hold invisible characters and be very long.
    for c in invisible_chars().into_iter().filter(|c| !c.is_whitespace()) {
        let s = scan(&format!("@foo{c}bar;"));
        assert_eq!(
            s.left[0].message, "@foobar is not read",
            "U+{:04X}",
            c as u32
        );
    }
    let s = scan(&format!("@{};", "a".repeat(100_000)));
    assert!(
        s.left[0].message.chars().count() < 120,
        "{} chars",
        s.left[0].message.chars().count()
    );
}

#[test]
fn what_a_scan_leaves_out_is_counted_and_only_the_first_are_kept() {
    // 60,000 rules that are not on the root: a note for each would be many times the input.
    let text = "a{}".repeat(60_000);
    let s = scan(&text);
    assert_eq!(s.left.len(), variables_import::scan::MAX_LEFT);
    assert_eq!(s.left_omitted, 60_000 - variables_import::scan::MAX_LEFT);
    assert!(retained(&s) < 200_000, "{} bytes kept", retained(&s));
}

#[test]
fn an_explicit_light_rule_inside_a_dark_query_is_skipped_but_the_root_is_the_dark_mode() {
    for sel in [".light", "[data-theme=light]", ":root.light", "html .light"] {
        let s = scan(&format!(
            "@media (prefers-color-scheme: dark) {{ {sel} {{ --a: 1 }} }}"
        ));
        assert!(s.decls.is_empty(), "{sel}");
        assert_eq!(codes(&s), ["light_in_dark_media"], "{sel}");
    }
    for sel in [":root", "html", "body"] {
        let s = scan(&format!(
            "@media (prefers-color-scheme: dark) {{ {sel} {{ --a: 1 }} }}"
        ));
        assert_eq!(
            kept(&s),
            [("--a", "1", Section::Root, false, true)],
            "{sel}"
        );
    }
}

#[test]
fn a_query_is_read_as_written_not_with_its_white_space_taken_out() {
    // `screenand(` is not `screen and (` in any browser.
    for q in [
        "screenand(prefers-color-scheme:dark)",
        "alland(prefers-color-scheme: dark)",
    ] {
        let s = scan(&format!("@media {q} {{ :root {{ --a: 1 }} }}"));
        assert!(s.decls.is_empty(), "{q}");
        assert_eq!(codes(&s), ["media_query_skipped"], "{q}");
    }
    // A comment between the words is white space.
    let s = scan("@media screen/**/and/**/(prefers-color-scheme: dark) { :root { --a: 1 } }");
    assert_eq!(kept(&s), [("--a", "1", Section::Root, false, true)]);
}
