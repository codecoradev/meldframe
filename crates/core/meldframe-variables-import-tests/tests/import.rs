//! What an import makes: collections, modes, variables and aliases, in an order the file accepts.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_protocol::{Command, Value};
use variables_import::EntryKind;

#[test]
fn root_properties_make_the_collection_root_with_a_light_mode() {
    let v = vars(":root { --background: #fff; --radius: 0.5rem; --brand: var(--background); }");
    assert_eq!(v.collections.len(), 1);
    let c = &v.collections[0];
    assert_eq!((c.id.as_str(), c.name.as_str()), ("col-root", "root"));
    assert_eq!(
        c.modes.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["light"]
    );
    assert_eq!(
        lit(&v, "root", "background", "light"),
        Value::Color("#fff".into())
    );
    assert_eq!(lit(&v, "root", "radius", "light"), Value::Number(8.0));
    assert_eq!(alias_of(&v, "root", "brand", "light"), "background");
}

#[test]
fn a_dark_rule_adds_the_dark_mode_and_a_variable_without_a_dark_value_inherits_light() {
    let i = run(":root { --a: #111; --b: #222; } .dark { --a: #eee; }");
    let v = applied(&i.commands).vars;
    let ids: Vec<_> = v.collections[0]
        .modes
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(ids, ["light", "dark"]);
    assert_eq!(lit(&v, "root", "a", "dark"), Value::Color("#eee".into()));
    assert_eq!(lit(&v, "root", "b", "dark"), Value::Color("#222".into()));
    let n = entries(&i, "inherited_dark");
    assert_eq!(n.len(), 1);
    assert!(n[0].message.starts_with("1 variable"), "{}", n[0].message);
    assert_eq!(n[0].kind, EntryKind::Note);
}

#[test]
fn the_selectors_and_queries_that_pick_the_dark_mode() {
    for rule in [
        ".dark { --a: #eee; }",
        ":root.dark { --a: #eee; }",
        "html.dark { --a: #eee; }",
        ".dark * { --a: #eee; }",
        "[data-theme=dark] { --a: #eee; }",
        "[data-theme=\"dark\"] { --a: #eee; }",
        ":root[data-mode='dark'] { --a: #eee; }",
        "@media (prefers-color-scheme: dark) { :root { --a: #eee; } }",
        "@media (prefers-color-scheme:dark){:root{--a:#eee}}",
        "@layer base { .dark { --a: #eee; } }",
    ] {
        let v = vars(&format!(":root {{ --a: #111; }} {rule}"));
        assert_eq!(v.collections[0].modes.len(), 2, "{rule}");
        assert_eq!(
            lit(&v, "root", "a", "dark"),
            Value::Color("#eee".into()),
            "{rule}"
        );
        assert_eq!(
            lit(&v, "root", "a", "light"),
            Value::Color("#111".into()),
            "{rule}"
        );
    }
}

#[test]
fn light_selectors_are_the_default_mode_and_a_list_sets_both() {
    let v =
        vars(":root, .dark { --a: #123; } .light { --b: #456; } :root:not(.dark) { --c: #789; }");
    assert_eq!(lit(&v, "root", "a", "light"), Value::Color("#123".into()));
    assert_eq!(lit(&v, "root", "a", "dark"), Value::Color("#123".into()));
    assert_eq!(lit(&v, "root", "b", "light"), Value::Color("#456".into()));
    assert_eq!(lit(&v, "root", "c", "light"), Value::Color("#789".into()));
}

#[test]
fn a_dark_only_property_has_no_default_value_and_is_skipped() {
    let i = run(":root { --a: #111; } .dark { --only-dark: #eee; }");
    assert_eq!(i.report.variables, 1);
    assert_eq!(
        skipped(&i, "--only-dark").unwrap().code,
        "dark_without_default"
    );
}

#[test]
fn theme_properties_make_the_collection_theme_with_tailwind_names() {
    let v = vars(
        "@theme { --color-brand-500: #3b82f6; --spacing: 0.25rem; --spacing-4: 1rem; \
         --radius-lg: 12px; --inset-shadow-sm: 1px; --Odd_Name.2: 3; --z-nav: 40; }",
    );
    assert_eq!(v.collections[0].name, "theme");
    assert_eq!(v.collections[0].modes[0].id, "default");
    assert_eq!(
        lit(&v, "theme", "color/brand-500", "default"),
        Value::Color("#3b82f6".into())
    );
    assert_eq!(lit(&v, "theme", "spacing", "default"), Value::Number(4.0));
    assert_eq!(
        lit(&v, "theme", "spacing/4", "default"),
        Value::Number(16.0)
    );
    assert_eq!(
        lit(&v, "theme", "radius/lg", "default"),
        Value::Number(12.0)
    );
    assert_eq!(
        lit(&v, "theme", "inset-shadow/sm", "default"),
        Value::Number(1.0)
    );
    assert_eq!(
        lit(&v, "theme", "odd-name-2", "default"),
        Value::Number(3.0)
    );
    assert_eq!(lit(&v, "theme", "z/nav", "default"), Value::Number(40.0));
}

#[test]
fn theme_forms_inline_static_and_reference_are_all_read() {
    for head in [
        "@theme",
        "@theme inline",
        "@theme static",
        "@theme inline reference",
    ] {
        let v = vars(&format!("{head} {{ --color-x: #fff; }}"));
        assert_eq!(v.variables.len(), 1, "{head}");
    }
}

#[test]
fn a_theme_alias_points_at_a_root_variable_in_another_collection() {
    let v = vars(
        "@theme inline { --color-background: var(--background); } \
         :root { --background: #fff; } .dark { --background: #000; }",
    );
    assert_eq!(v.collections.len(), 2);
    assert_eq!(
        alias_of(&v, "theme", "color/background", "default"),
        "background"
    );
}

#[test]
fn aliases_come_after_what_they_point_at_whatever_the_order_in_the_text() {
    let i = run(":root { --c: var(--b); --b: var(--a); --a: #abc; }");
    let names: Vec<_> = i
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::AddVariable { variable } => Some(variable.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, ["a", "b", "c"]);
    applied(&i.commands);
    assert_eq!(i.report.aliases, 2);
}

#[test]
fn an_alias_keeps_its_type_from_its_target_and_a_fallback_is_reported_not_kept() {
    let i = run(":root { --r: 4px; --x: var(--r, 9px); }");
    let v = applied(&i.commands).vars;
    assert_eq!(
        v.variable("var-root-x").unwrap().kind,
        meldframe_protocol::VarType::Number
    );
    assert_eq!(entries(&i, "fallback_ignored").len(), 1);
}
