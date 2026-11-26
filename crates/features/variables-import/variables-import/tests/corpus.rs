//! The representative theme (`fixtures/representative-theme.css`), end to end: every custom property ends imported
//! or skipped with a reason, the commands apply to an empty file, and the dark mode, the aliases and the conversions
//! are what the text says. The fixture is written for these tests; it is not the owner's own theme.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_protocol::Value;

const THEME: &str = include_str!("fixtures/representative-theme.css");

#[test]
fn the_theme_imports_with_a_report_that_accounts_for_every_property() {
    let i = run(THEME);
    let doc = applied(&i.commands);
    let r = &i.report;
    assert_eq!((r.collections, r.modes), (2, 3));
    assert_eq!(r.variables, doc.vars.variables.len());
    assert_eq!(r.colors + r.numbers, r.variables);
    assert_eq!(
        (r.variables, r.colors, r.numbers, r.aliases, r.skipped),
        (EXPECTED.0, EXPECTED.1, EXPECTED.2, EXPECTED.3, EXPECTED.4),
        "{:#?}",
        r.entries
    );
    assert_eq!(r.omitted, 0);
}

/// variables, colours, numbers, aliases, skipped.
const EXPECTED: (usize, usize, usize, usize, usize) = (47, 37, 10, 10, 9);

#[test]
fn the_dark_mode_and_the_aliases_of_the_theme_are_as_written() {
    let v = vars(THEME);
    let root: Vec<_> = v
        .collections
        .iter()
        .filter(|c| c.id == "col-root")
        .collect();
    assert_eq!(
        root[0]
            .modes
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        ["light", "dark"]
    );
    assert_eq!(
        lit(&v, "root", "background", "light"),
        Value::Color("#ffffff".into())
    );
    assert_eq!(
        lit(&v, "root", "background", "dark"),
        Value::Color("#0a0a0a".into())
    );
    assert_eq!(
        lit(&v, "root", "overlay", "light"),
        Value::Color("#00000080".into())
    );
    assert_eq!(
        lit(&v, "root", "overlay", "dark"),
        Value::Color("#000000b3".into())
    );
    assert_eq!(
        lit(&v, "root", "surface-raised", "light"),
        Value::Color("#ffffff".into())
    );
    assert_eq!(lit(&v, "root", "radius", "light"), Value::Number(10.0));
    assert_eq!(alias_of(&v, "root", "brand", "light"), "color/brand-600");
    assert_eq!(alias_of(&v, "root", "brand", "dark"), "color/brand-400");
    assert_eq!(
        alias_of(&v, "theme", "color/background", "default"),
        "background"
    );
    assert_eq!(alias_of(&v, "theme", "color/brand", "default"), "brand");
    assert_eq!(alias_of(&v, "theme", "radius/lg", "default"), "radius");
    assert_eq!(
        lit(&v, "theme", "color/brand-500", "default"),
        Value::Color("#3b82f6".into())
    );
    assert_eq!(lit(&v, "theme", "spacing", "default"), Value::Number(4.0));
    assert_eq!(
        lit(&v, "theme", "breakpoint/md", "default"),
        Value::Number(768.0)
    );
}

#[test]
fn what_the_theme_has_that_the_model_cannot_hold_is_skipped_by_name_with_its_reason() {
    let i = run(THEME);
    for (name, code) in [
        ("--shadow-ring", "computed_value"),
        ("--nav-height", "computed_value"),
        ("--font-body", "unsupported_type"),
        ("--radius-sm", "computed_value"),
        ("--font-sans", "unsupported_type"),
        ("--shadow-card", "unsupported_type"),
        ("--ease-snappy", "unsupported_type"),
        ("--animate-fade", "unsupported_type"),
        ("--hero-glow", "dark_without_default"),
    ] {
        assert_eq!(
            skipped(&i, name).unwrap_or_else(|| panic!("{name}")).code,
            code,
            "{name}"
        );
    }
    assert_eq!(entries(&i, "import_skipped").len(), 2);
    assert!(
        entries(&i, "at_rule_skipped")
            .iter()
            .any(|e| e.name.contains("@plugin"))
    );
    assert!(
        i.report
            .entries
            .iter()
            .any(|e| e.code == "selector_skipped" && e.name == ".card")
    );
    assert!(
        skipped(&i, "--card-padding").is_none(),
        "a rule that is not the root is reported by its selector"
    );
}

#[test]
fn every_conversion_of_a_colour_is_in_the_report() {
    let i = run(THEME);
    let converted = entries(&i, "converted");
    let named = |n: &str| converted.iter().any(|e| e.name == n);
    assert!(
        named("--background") && named("--chart-1") && named("--overlay") && named("--sidebar")
    );
    assert!(named("--color-accent"));
    assert!(
        !named("--color-brand-50"),
        "a hex colour is not changed, so it is not reported"
    );
    let clipped = entries(&i, "clipped");
    // --destructive is a little outside sRGB in both modes, --chart-2 is far outside it.
    let names: Vec<_> = clipped.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["--destructive", "--destructive", "--chart-2"]);
}

#[test]
fn the_dark_selector_is_named_in_the_report() {
    let i = run(THEME);
    let n = entries(&i, "dark_mode_source");
    assert_eq!(n.len(), 1);
    assert!(n[0].message.contains(".dark"), "{}", n[0].message);
}
