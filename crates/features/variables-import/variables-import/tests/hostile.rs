//! Hostile text: the scan never panics, reads in one pass and keeps what it makes bounded.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use variables_import::{ImportError, ImportOptions, MAX_INPUT_BYTES, import_css};

#[test]
fn text_over_the_limit_is_refused_with_its_code() {
    let big = " ".repeat(MAX_INPUT_BYTES + 1);
    let e = import_css(&big, &ImportOptions::default()).unwrap_err();
    assert_eq!(e.code(), "E_IMPORT_TOO_LARGE");
    assert!(
        matches!(e, ImportError::TooLarge { bytes, max } if bytes == MAX_INPUT_BYTES + 1 && max == MAX_INPUT_BYTES)
    );
    let at = import_css(&" ".repeat(MAX_INPUT_BYTES), &ImportOptions::default());
    assert!(at.unwrap().commands.is_empty());
}

/// Broken, truncated and hostile text, and exactly how many variables each makes.
const CUTS: [(&str, usize); 33] = [
    ("", 0),
    ("{", 0),
    ("}", 0),
    ("}}}}", 0),
    (";;;;", 0),
    ("@", 0),
    ("@{", 0),
    (":root {", 0),
    (":root { --a", 0),
    (":root { --a:", 0),
    (":root { --a: \"x", 0),
    (":root { --a: url(", 0),
    ("/*", 0),
    ("/* x", 0),
    (":root { /* --a: #fff; */ }", 0),
    ("@media { { { {", 0),
    (":root { --a: #fff; } }", 1),
    ("--a: #fff;", 0),
    (":root { --a: '\\", 0),
    (":root { --a: \"\\\u{e9}", 0),
    ("\u{e9}\u{e9} { --a: 1 }", 0),
    (":root { --\u{e9}: \u{1f600}; }", 0),
    (":root{--a:#fff}.dark{--a:}", 0),
    (":root { --a: var(; }", 0),
    (":root { --a: var(--); }", 0),
    (":root { --a: var(--b, ); --b: #fff }", 2),
    (":root { --a: rgb(; }", 0),
    (":root { --a: oklch(NaN 1e999 inf); }", 0),
    (":root { --a: 1e999px; }", 0),
    (":root { --a: -; --b: +; --c: .; }", 0),
    (":root { --a: hsl(1e308 100% 50%); }", 0),
    (":root { --a: rgb(1e308 0 0); }", 1),
    (":root { --a: oklch(1e308 1e308 1e308); }", 0),
];

#[test]
fn broken_text_gives_exactly_what_it_says_and_never_panics() {
    for (text, variables) in CUTS {
        let i = run(text);
        applied(&i.commands);
        assert_eq!(
            i.report.variables, variables,
            "{text:?}: {:?}",
            i.report.entries
        );
    }
}

#[test]
fn every_prefix_of_the_theme_imports_without_panicking_and_never_more_than_the_whole() {
    let t = include_str!("fixtures/representative-theme.css");
    let whole = run(t).report.variables;
    for end in (0..=t.len()).filter(|e| t.is_char_boundary(*e)) {
        let i = run(&t[..end]);
        applied(&i.commands);
        assert!(i.report.variables <= whole, "prefix of {end} bytes");
    }
}

#[test]
fn more_custom_properties_than_the_scan_keeps_are_reported() {
    let mut t = String::from(":root {");
    for k in 0..20_100 {
        t.push_str(&format!("--v{k}:1;"));
    }
    t.push('}');
    let i = run(&t);
    assert_eq!(i.report.variables, 5_000);
    assert_eq!(entries(&i, "limit_declarations").len(), 1);
}

#[test]
fn no_code_runs_and_nothing_is_opened() {
    // These are only text: nothing in them is evaluated, fetched or followed.
    let i = run(
        "@import url(file:///etc/passwd); :root { --a: url(http://169.254.169.254/); --b: calc(1px + 1px); \
                 --c: expression(alert(1)); --d: #fff; }",
    );
    assert_eq!(i.report.variables, 1);
    assert_eq!(i.report.skipped, 3);
}
