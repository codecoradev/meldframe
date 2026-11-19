//! The report and the names of an import: what is said about what was changed, and what the same text always gives.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_protocol::Value;

#[test]
fn the_same_text_makes_the_same_commands_and_ids() {
    let text =
        ":root { --a: #111; --b: var(--a); } .dark { --a: #eee; } @theme { --color-c: #fff; }";
    let (x, y) = (run(text), run(text));
    assert_eq!(format!("{:?}", x.commands), format!("{:?}", y.commands));
    let ids: Vec<_> = applied(&x.commands)
        .vars
        .variables
        .iter()
        .map(|v| v.id.clone())
        .collect();
    assert_eq!(ids, ["var-root-a", "var-theme-color-c", "var-root-b"]);
}

#[test]
fn a_later_declaration_in_the_same_mode_wins_and_is_noted() {
    let i = run(":root { --a: #111; } :root { --a: #222; }");
    let v = applied(&i.commands).vars;
    assert_eq!(lit(&v, "root", "a", "light"), Value::Color("#222".into()));
    assert_eq!(entries(&i, "overridden").len(), 1);
}

#[test]
fn a_name_declared_in_theme_and_root_is_kept_once() {
    let i = run("@theme { --color-x: #111; } :root { --color-x: #222; }");
    assert_eq!(i.report.variables, 1);
    assert_eq!(skipped(&i, "--color-x").unwrap().code, "duplicate_name");
}

#[test]
fn rem_is_sixteen_pixels_and_is_reported() {
    let i = run(":root { --a: 1.5rem; --b: 12px; --c: 0.5; --d: -4px; }");
    let v = applied(&i.commands).vars;
    assert_eq!(lit(&v, "root", "a", "light"), Value::Number(24.0));
    assert_eq!(lit(&v, "root", "b", "light"), Value::Number(12.0));
    assert_eq!(lit(&v, "root", "c", "light"), Value::Number(0.5));
    assert_eq!(lit(&v, "root", "d", "light"), Value::Number(-4.0));
    assert_eq!(entries(&i, "rem").len(), 1);
}

#[test]
fn a_name_that_had_to_change_is_reported() {
    let i = run(":root { --Brand_Blue: #00f; --plain: #000; }");
    let e = entries(&i, "renamed");
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].name, "--Brand_Blue");
    assert!(e[0].message.contains("brand-blue"));
}

#[test]
fn two_names_that_become_one_are_not_both_imported() {
    let i = run(":root { --a_b: #111; --a-b: #222; }");
    assert_eq!(i.report.variables, 1);
    assert_eq!(skipped(&i, "--a-b").unwrap().code, "name_collision");
}
