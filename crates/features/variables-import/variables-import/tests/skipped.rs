//! What an import leaves out, and the report that says so: nothing is dropped without a line.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_variables::MAX_ALIAS_DEPTH;

#[test]
fn import_and_url_are_reported_as_skipped_and_never_followed() {
    let i = run(
        "@import \"tailwindcss\"; @import url(\"https://x.example/a.css\"); :root { --a: #111; \
                 --bg: url(\"https://x.example/i.png\"); }",
    );
    assert_eq!(i.report.variables, 1);
    assert_eq!(entries(&i, "import_skipped").len(), 2);
    assert!(entries(&i, "import_skipped")[1].name.contains("x.example"));
    assert_eq!(skipped(&i, "--bg").unwrap().code, "url_skipped");
}

#[test]
fn rules_that_are_not_the_root_and_other_at_rules_are_skipped_with_their_selector() {
    let i = run(
        ".card { --pad: 4px; } @keyframes x { from { --a: 1 } } @container (min-width: 1px) { :root { --b: 1 } } \
         @supports (color: red) { :root { --c: #fff } } :root { --ok: #fff; }",
    );
    assert_eq!(i.report.variables, 1);
    let codes: Vec<_> = i.report.entries.iter().map(|e| e.code).collect();
    assert!(codes.contains(&"selector_skipped"), "{codes:?}");
    assert!(codes.contains(&"at_rule_skipped"), "{codes:?}");
    assert!(i.report.entries.iter().any(|e| e.name == ".card"));
}

#[test]
fn an_alias_to_a_missing_or_skipped_target_is_skipped_and_so_are_the_ones_that_follow_it() {
    let i = run(
        ":root { --a: var(--nope); --b: var(--a); --c: calc(1px + 1px); --d: var(--c); --ok: #000; }",
    );
    assert_eq!(i.report.variables, 1);
    for n in ["--a", "--b", "--d"] {
        assert_eq!(skipped(&i, n).unwrap().code, "alias_target_missing", "{n}");
    }
    assert_eq!(i.report.skipped, 4);
}

#[test]
fn an_alias_cycle_is_skipped_whole() {
    let i =
        run(":root { --a: var(--b); --b: var(--c); --c: var(--a); --d: var(--a); --ok: #000; }");
    assert_eq!(i.report.variables, 1);
    for n in ["--a", "--b", "--c", "--d"] {
        assert_eq!(skipped(&i, n).unwrap().code, "alias_cycle", "{n}");
    }
    let s = run(":root { --self: var(--self); }");
    assert_eq!(skipped(&s, "--self").unwrap().code, "alias_cycle");
}

#[test]
fn an_alias_of_another_type_and_a_variable_of_two_types_are_skipped() {
    let i = run(
        ":root { --n: 4px; --c: #fff; --x: var(--n); --mix: var(--c); } .dark { --mix: var(--n); --lit: 4px; } :root { --lit: #000; }",
    );
    assert_eq!(skipped(&i, "--mix").unwrap().code, "type_mismatch");
    assert_eq!(skipped(&i, "--lit").unwrap().code, "type_mismatch");
    assert!(skipped(&i, "--x").is_none());
}

#[test]
fn a_chain_the_file_would_refuse_is_skipped_and_the_longest_the_file_takes_is_not() {
    // `--v0` is a literal; `--v{k}` aliases `--v{k-1}`, so `--v{k}` is k alias steps from a value.
    let build = |n: usize| {
        let mut t = String::from(":root { --v0: #fff; ");
        for k in 1..=n {
            t.push_str(&format!("--v{k}: var(--v{}); ", k - 1));
        }
        t.push('}');
        run(&t)
    };
    let top = MAX_ALIAS_DEPTH;
    let ok = build(top);
    applied(&ok.commands);
    assert_eq!(ok.report.skipped, 0);
    let over = build(top + 3);
    assert_eq!(over.report.skipped, 3);
    assert_eq!(
        skipped(&over, &format!("--v{}", top + 1)).unwrap().code,
        "alias_too_deep"
    );
    applied(&over.commands);
}

#[test]
fn the_limits_of_a_file_hold_and_what_is_over_is_reported() {
    let mut t = String::from(":root {");
    for k in 0..5_010 {
        t.push_str(&format!("--v{k}: {k}px;"));
    }
    t.push('}');
    let i = run(&t);
    assert_eq!(i.report.variables, 5_000);
    assert_eq!(i.report.skipped, 10);
    assert!(i.report.omitted > 0 || i.report.entries.len() == 10);
    let big = run(":root { --n: 1000000px; --ok: 99999px; }");
    assert_eq!(skipped(&big, "--n").unwrap().code, "out_of_range");
    assert_eq!(big.report.variables, 1);
    applied(&i.commands);
}

#[test]
fn a_report_is_bounded_and_counts_what_it_left_out() {
    let mut t = String::from(":root {");
    for k in 0..500 {
        t.push_str(&format!("--v{k}: calc({k}px + 1px);"));
    }
    t.push('}');
    let i = run(&t);
    assert_eq!(i.report.skipped, 500);
    assert_eq!(i.report.entries.len(), variables_import::MAX_ENTRIES);
    assert_eq!(i.report.omitted, 300);
}

#[test]
fn a_long_name_and_an_empty_one_are_skipped() {
    let long = "a".repeat(300);
    let i = run(&format!(
        ":root {{ --{long}: #fff; ---: #000; --: #111; --\u{e9}: #222; }}"
    ));
    assert_eq!(i.report.variables, 0);
    let codes: Vec<_> = i.report.entries.iter().map(|e| e.code).collect();
    assert_eq!(
        codes,
        [
            "name_too_long",
            "invalid_name",
            "invalid_name",
            "invalid_name"
        ]
    );
}
