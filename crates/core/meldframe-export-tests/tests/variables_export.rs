//! D8: every variable of a file as CSS custom properties, one declaration per line (`export_variables`). The same
//! names and mode blocks as the variables of a frame export (ADR-0019), for all variables and not only the ones a
//! frame uses.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod support;
use export::{ExportError, Format, MAX_OUTPUT_BYTES, export_variables};
use meldframe_protocol::Variables;
use meldframe_render::ModeChoice;
use serde_json::json;
use support::*;

fn fixture() -> Variables {
    serde_json::from_str(include_str!("fixtures/variables-export.json")).unwrap()
}

/// The shape every output has, whatever the file holds: rules `selector {`, declarations `  --name: value;` and `}`, with
/// a name of lowercase letters, digits and hyphens, a value that is a hex colour, `transparent`, `var(--name)` of a name
/// that is declared, or a number, and braces that balance. Returns the declared names (each once).
fn wellformed(css: &str) -> std::collections::HashSet<String> {
    let mut declared = std::collections::HashSet::new();
    let mut open = false;
    let mut aliases = Vec::new();
    for line in css.lines() {
        if let Some(decl) = line.strip_prefix("  ") {
            assert!(open, "a declaration outside a rule: {line:?}");
            let decl = decl
                .strip_suffix(';')
                .unwrap_or_else(|| panic!("no semicolon: {line:?}"));
            let (name, value) = decl
                .split_once(": ")
                .unwrap_or_else(|| panic!("no colon: {line:?}"));
            let ident = |s: &str| {
                !s.is_empty()
                    && s.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            };
            assert!(
                name.starts_with("--") && ident(&name[2..]),
                "bad name: {line:?}"
            );
            if let Some(target) = value.strip_prefix("var(").and_then(|v| v.strip_suffix(')')) {
                assert!(
                    target.starts_with("--") && ident(&target[2..]),
                    "bad alias: {line:?}"
                );
                aliases.push(target.to_string());
            } else if let Some(hex) = value.strip_prefix('#') {
                assert!(
                    matches!(hex.len(), 3 | 4 | 6 | 8)
                        && hex.bytes().all(|b| b.is_ascii_hexdigit()),
                    "bad hex: {line:?}"
                );
            } else {
                let number = value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'));
                assert!(value == "transparent" || number, "bad value: {line:?}");
            }
            declared.insert(name.to_string());
        } else if line == "}" {
            assert!(open, "a closing brace outside a rule");
            open = false;
        } else {
            let selector = line
                .strip_suffix(" {")
                .unwrap_or_else(|| panic!("not a rule line: {line:?}"));
            assert!(!open, "a rule inside a rule: {line:?}");
            let ok = selector == ":root"
                || selector
                    .strip_prefix(":root[data-mode-")
                    .and_then(|r| r.strip_suffix("\"]"))
                    .and_then(|r| r.split_once("=\""))
                    .is_some_and(|(c, m)| {
                        c.bytes()
                            .chain(m.bytes())
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                    });
            assert!(ok, "bad selector: {line:?}");
            open = true;
        }
    }
    assert!(!open, "a rule that is not closed");
    assert!(
        !css.contains("/*") && !css.contains("*/"),
        "a comment opens inside the text"
    );
    for target in aliases {
        assert!(declared.contains(&target), "{target} is not declared");
    }
    declared
}

#[test]
fn the_fixture_gives_the_expected_text_byte_for_byte() {
    let e = export_variables(&fixture()).unwrap();
    assert_eq!(e.css, include_str!("fixtures/variables-export.css"));
    assert_eq!((e.collections, e.variables, e.blocks), (2, 5, 2));
    assert!(e.warnings.is_empty(), "{:?}", e.warnings);
}

#[test]
fn it_is_deterministic() {
    let v = fixture();
    assert_eq!(export_variables(&v), export_variables(&v));
}

#[test]
fn a_file_without_variables_exports_nothing() {
    let e = export_variables(&Variables::default()).unwrap();
    assert_eq!(e.css, "");
    assert_eq!((e.collections, e.variables, e.blocks), (0, 0, 0));
}

#[test]
fn the_names_are_those_of_a_frame_export_and_a_variable_the_frame_does_not_use_is_still_there() {
    let v = fixture();
    let frame = nodes(frame(
        json!({"fill": {"var": "v4"}}),
        vec![json!({"id": "r", "type": "rect", "radius": {"var": "v2"}})],
    ));
    let framed = run(&frame, &v, Format::Css, ModeChoice::default());
    let all = export_variables(&v).unwrap().css;
    for name in [
        "--appearance-color-brand",
        "--appearance-color-accent",
        "--sizes-radius-card",
    ] {
        assert!(framed.css.contains(&format!("{name}:")), "{}", framed.css);
        assert!(all.contains(&format!("  {name}: ")), "{all}");
    }
    assert!(!framed.css.contains("unused"), "the frame leaves it out");
    assert!(all.contains("--appearance-color-unused"), "{all}");
}

#[test]
fn export_variables_keeps_the_stored_hex_with_its_alpha() {
    let all = export_variables(&fixture()).unwrap().css;
    assert!(
        all.contains("--appearance-color-unused: #ffffff80;"),
        "{all}"
    );
}

#[test]
fn a_hex_colour_is_written_lowercased_with_its_alpha() {
    let v: Variables = serde_json::from_value(json!({
        "collections": [{"id": "c", "name": "a", "modes": [{"id": "m1", "name": "A"}]}],
        "variables": [
            {"id": "v1", "collection": "c", "name": "x", "type": "color", "values": {"m1": "#AABBCC80"}},
            {"id": "v2", "collection": "c", "name": "y", "type": "color", "values": {"m1": "#ABC"}}
        ]
    }))
    .unwrap();
    let css = export_variables(&v).unwrap().css;
    assert!(css.contains("--a-x: #aabbcc80;"), "{css}");
    assert!(css.contains("--a-y: #abc;"), "{css}");
}

#[test]
fn a_variable_without_a_value_in_a_mode_takes_the_default_mode_and_a_bad_colour_is_reported() {
    let v: Variables = serde_json::from_value(json!({
        "collections": [{"id": "c", "name": "a", "modes": [{"id": "m1", "name": "A"}, {"id": "m2", "name": "B"}]}],
        "variables": [
            {"id": "v1", "collection": "c", "name": "x", "type": "color", "values": {"m1": "#111111"}},
            {"id": "v2", "collection": "c", "name": "y", "type": "color", "values": {"m1": "rebeccapurple"}},
            {"id": "v3", "collection": "c", "name": "z", "type": "color", "values": {"m2": "#222222"}},
            {"id": "v4", "collection": "c", "name": "w", "type": "color", "values": {"m1": {"var": "gone"}}}
        ]
    }))
    .unwrap();
    let e = export_variables(&v).unwrap();
    assert!(
        e.css
            .contains(":root[data-mode-a=\"b\"] {\n  --a-x: #111111;\n"),
        "{}",
        e.css
    );
    assert!(e.css.contains("--a-y: transparent;"), "{}", e.css);
    let codes: Vec<&str> = e.warnings.iter().map(|w| w.code).collect();
    assert!(codes.contains(&"invalid_colour"), "{codes:?}");
    assert!(codes.contains(&"missing_value"), "{codes:?}");
    assert!(codes.contains(&"unresolved_alias"), "{codes:?}");
}

#[test]
fn an_output_over_the_limit_is_refused_with_the_export_code() {
    let n = MAX_OUTPUT_BYTES / 15;
    let variables: Vec<_> = (0..n)
        .map(|i| json!({"id": format!("v{i}"), "collection": "c", "name": format!("name-{i}"), "type": "number", "values": {"m": 1}}))
        .collect();
    let v: Variables = serde_json::from_value(json!({
        "collections": [{"id": "c", "name": "big", "modes": [{"id": "m", "name": "Default"}]}],
        "variables": variables
    }))
    .unwrap();
    let e = export_variables(&v).unwrap_err();
    assert!(matches!(e, ExportError::TooLarge { .. }), "{e:?}");
    assert_eq!(e.code(), "E_EXPORT_TOO_LARGE");
}

/// The contract: for any file, every variable has one declaration in `:root` and one in the block of each other mode of
/// its collection, every name is unique, every line has the one form `  --name: value;`, and every alias points at a
/// name that is declared.
#[test]
fn the_contract_one_declaration_for_each_variable_and_mode_and_every_alias_defined() {
    let mut collections = Vec::new();
    let mut variables = Vec::new();
    for c in 0..4 {
        let modes: Vec<_> = (0..=c)
            .map(|m| json!({"id": format!("m{c}-{m}"), "name": format!("Mode {m}")}))
            .collect();
        collections.push(
            json!({"id": format!("c{c}"), "name": format!("Collection {c}"), "modes": modes}),
        );
        for i in 0..6 {
            let values: serde_json::Map<String, serde_json::Value> = (0..=c)
                .map(|m| {
                    let value = match (i + m) % 3 {
                        0 => json!(format!("#{:06x}", c * 1000 + i * 10 + m)),
                        1 if i > 0 => json!({"var": format!("v{c}-{}", i - 1)}),
                        _ => json!(i as f64 + 0.5),
                    };
                    (format!("m{c}-{m}"), value)
                })
                .collect();
            variables.push(json!({"id": format!("v{c}-{i}"), "collection": format!("c{c}"), "name": format!("group/name {i}"), "type": "color", "values": values}));
        }
    }
    let v: Variables =
        serde_json::from_value(json!({"collections": collections, "variables": variables}))
            .unwrap();
    let e = export_variables(&v).unwrap();
    assert!(e.warnings.is_empty(), "{:?}", e.warnings);
    let declared = wellformed(&e.css);
    let lines = e.css.lines().filter(|l| l.starts_with("  ")).count();
    // 6 variables in each of the collections of 1, 2, 3 and 4 modes.
    assert_eq!(lines, 6 * (1 + 2 + 3 + 4));
    assert_eq!(declared.len(), 24, "every name is unique");
    for line in e.css.lines().filter(|l| l.contains("var(--")) {
        let target = line.split("var(").nth(1).unwrap().trim_end_matches(");");
        assert!(
            declared.contains(target),
            "{line} points at nothing declared"
        );
    }
    assert_eq!(e.blocks, 1 + 1 + 2 + 3);
}

#[test]
fn names_that_try_to_leave_their_declaration_or_their_block_stay_inside_it() {
    // What a stored file could hold even though the commands refuse it: braces, semicolons, line breaks, comments,
    // quotes, direction overrides, other scripts. The slug of a name keeps ASCII letters and digits only.
    let bad = "}\n;x/*\"'\u{202e}\u{0000}é語{--evil:red}";
    let v: Variables = serde_json::from_value(json!({
        "collections": [{"id": "c", "name": bad, "modes": [{"id": "m1", "name": "a"}, {"id": "m2", "name": bad}]}],
        "variables": [
            {"id": "v1", "collection": "c", "name": bad, "type": "color", "values": {"m1": "#112233", "m2": "red;} body{display:none"}},
            {"id": "v2", "collection": "c", "name": format!("{bad}2"), "type": "color", "values": {"m1": {"var": "v1"}, "m2": "url(javascript:alert(1))"}},
            {"id": "v3", "collection": "c", "name": "n", "type": "number", "values": {"m1": 1.5, "m2": -0.0}}
        ]
    }))
    .unwrap();
    let e = export_variables(&v).unwrap();
    wellformed(&e.css);
    // The letters of a name survive as a slug (`evil-red`), its punctuation does not; a value that is not hex is transparent.
    assert!(
        !e.css.contains("display:none") && !e.css.contains("javascript") && !e.css.contains("{--"),
        "{}",
        e.css
    );
    assert!(e.css.contains("-x-evil-red: #112233;"), "{}", e.css);
    assert_eq!(
        e.warnings
            .iter()
            .filter(|w| w.code == "invalid_colour")
            .count(),
        2
    );
}

#[test]
fn numbers_that_are_not_finite_stay_inside_their_declaration() {
    let mut v = fixture();
    for var in &mut v.variables {
        if let Some(meldframe_protocol::Bind::Lit(meldframe_protocol::Value::Number(n))) =
            var.values.values_mut().next()
        {
            *n = f64::NAN;
        }
    }
    wellformed(&export_variables(&v).unwrap().css);
}

/// The declarations of `css` by rule, in order: from the compact rules of a frame export (`:root{a:b;c:d}`).
fn compact(css: &str) -> Vec<(String, Vec<String>)> {
    css.lines()
        .filter(|l| l.starts_with(":root"))
        .map(|l| {
            let (sel, rest) = l.split_once('{').unwrap();
            let body = rest.strip_suffix('}').unwrap();
            (
                sel.to_string(),
                body.split(';')
                    .filter(|d| !d.is_empty())
                    .map(String::from)
                    .collect(),
            )
        })
        .collect()
}

/// The same from the lines of `export_variables`.
fn pretty(css: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for line in css.lines() {
        if let Some(sel) = line.strip_suffix(" {") {
            out.push((sel.to_string(), Vec::new()));
        } else if let Some(d) = line.strip_prefix("  ") {
            out.last_mut()
                .unwrap()
                .1
                .push(d.trim_end_matches(';').replacen(": ", ":", 1));
        }
    }
    out
}

fn bound_file() -> (Vec<meldframe_protocol::Node>, Variables) {
    let v: Variables = serde_json::from_value(json!({
        "collections": [
            {"id": "c1", "name": "appearance", "modes": [{"id": "m1", "name": "Light"}, {"id": "m2", "name": "Dark"}]},
            {"id": "c2", "name": "sizes", "modes": [{"id": "m3", "name": "Default"}]}
        ],
        "variables": [
            {"id": "v1", "collection": "c1", "name": "color/brand", "type": "color", "values": {"m1": "#112233", "m2": "#aabbcc"}},
            {"id": "v2", "collection": "c2", "name": "radius/card", "type": "number", "values": {"m3": 12}},
            {"id": "v3", "collection": "c2", "name": "opacity/muted", "type": "number", "values": {"m3": 0.4}},
            {"id": "v4", "collection": "c1", "name": "color/accent", "type": "color", "values": {"m1": {"var": "v1"}, "m2": "#000000"}}
        ]
    }))
    .unwrap();
    let n = nodes(frame(
        json!({"fill": {"var": "v4"}}),
        vec![json!({"id": "r", "type": "rect", "radius": {"var": "v2"}, "opacity": {"var": "v3"}})],
    ));
    (n, v)
}

#[test]
fn the_lines_are_those_of_the_frame_export_for_the_same_variables() {
    // The frame uses v4, v2 and v3, and v4 reaches v1: every variable of the file is in the frame export.
    let (n, v) = bound_file();
    let framed = run(&n, &v, Format::Css, ModeChoice::default());
    assert_eq!(
        compact(&framed.css),
        pretty(&export_variables(&v).unwrap().css)
    );
}

#[test]
fn a_collision_of_names_is_numbered_in_file_order_so_it_can_differ_from_a_frame_export() {
    // `a/b` and `a-b` are both `--c-a-b`. The frame export numbers the variables the frame uses, so when only the second
    // is used it is the first of its name there and `--c-a-b` in the frame, `--c-a-b-2` in the file of all variables.
    let v: Variables = serde_json::from_value(json!({
        "collections": [{"id": "c", "name": "c", "modes": [{"id": "m", "name": "Default"}]}],
        "variables": [
            {"id": "v1", "collection": "c", "name": "a/b", "type": "number", "values": {"m": 1}},
            {"id": "v2", "collection": "c", "name": "a-b", "type": "number", "values": {"m": 2}}
        ]
    }))
    .unwrap();
    let n = nodes(frame(json!({"opacity": {"var": "v2"}}), vec![]));
    let framed = run(&n, &v, Format::Css, ModeChoice::default());
    assert!(framed.css.contains(":root{--c-a-b:2}"), "{}", framed.css);
    let all = export_variables(&v).unwrap().css;
    assert!(all.contains("  --c-a-b: 1;\n  --c-a-b-2: 2;\n"), "{all}");
}
