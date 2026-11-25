//! Values: `var()` replaced by the literal, lengths and colours read, a value cut into words, and the shorthands
//! expanded to the longhands the mapping reads.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use html_import::css::value::{Unit, colour, has_top_level_comma, is_wide, length, words};
use html_import::css::vars::{MAX_DEPTH, MAX_EXPANSIONS, VarError, resolve};
use std::collections::HashMap;
use std::sync::Arc;

fn vars(pairs: &[(&str, &str)]) -> HashMap<String, Arc<str>> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), (*v).into()))
        .collect()
}

fn res(value: &str, pairs: &[(&str, &str)]) -> Result<String, VarError> {
    let m = vars(pairs);
    resolve(value, &|n| m.get(n).cloned(), 1_000)
}

#[test]
fn a_var_is_replaced_by_its_value_and_a_fallback_by_what_is_missing() {
    let v = [
        ("--a", "#fff"),
        ("--n", "8"),
        ("--empty", ""),
        ("--b", "var(--a)"),
    ];
    for (text, want) in [
        ("var(--a)", "#fff"),
        ("1px solid var(--a)", "1px solid #fff"),
        ("var( --a )", "#fff"),
        ("VAR(--a)", "#fff"),
        ("var(--b)", "#fff"),
        ("var(--none, red)", "red"),
        ("var(--none, var(--a))", "#fff"),
        ("var(--none,)", ""),
        ("var(--a, red)", "#fff"),
        ("calc(var(--n) * 1px)", "calc(8 * 1px)"),
        ("var(--n) var(--n)", "8 8"),
        ("a var(--empty) b", "a  b"),
        ("var(--none, rgb(0, 0, 0))", "rgb(0, 0, 0)"),
        // A string is text, and a name that only ends in `var(` is not one.
        ("\"var(--a)\" 'var(--a)'", "\"var(--a)\" 'var(--a)'"),
        ("myvar(--a) x-var(--a)", "myvar(--a) x-var(--a)"),
        ("no var here", "no var here"),
        ("", ""),
    ] {
        assert_eq!(res(text, &v).as_deref(), Ok(want), "{text}");
    }
}

#[test]
fn what_cannot_be_resolved_leaves_the_value_unresolved_with_its_reason() {
    let v = [("--a", "var(--b)"), ("--b", "var(--a)"), ("--c", "x")];
    assert_eq!(
        res("var(--none)", &v),
        Err(VarError::Undefined("--none".into()))
    );
    assert_eq!(res("var(--a)", &v), Err(VarError::TooDeep));
    assert_eq!(res("var(--c", &v), Err(VarError::Syntax));
    assert_eq!(res("var(c)", &v), Err(VarError::Syntax));
    assert_eq!(res("var(--)", &v), Err(VarError::Syntax));
    assert_eq!(VarError::TooDeep.code(), "var_too_deep");
    // A chain is as long as it is: MAX_DEPTH steps pass, one more does not.
    let chain = |n: usize| -> Vec<(String, String)> {
        let mut c: Vec<(String, String)> = (0..n)
            .map(|i| (format!("--v{i}"), format!("var(--v{})", i + 1)))
            .collect();
        c.push((format!("--v{n}"), "end".into()));
        c
    };
    let run = |n: usize| {
        let c = chain(n);
        let pairs: Vec<(&str, &str)> = c.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        res("var(--v0)", &pairs)
    };
    assert_eq!(run(MAX_DEPTH - 1).as_deref(), Ok("end"));
    assert_eq!(run(MAX_DEPTH + 1), Err(VarError::TooDeep));
}

#[test]
fn an_expansion_is_bounded_in_bytes_and_in_count() {
    // 2^10 replacements of an empty value would cost nothing in bytes: the count stops them.
    let c: Vec<(String, String)> = (0..10)
        .map(|i| {
            (
                format!("--e{i}"),
                format!("var(--e{}) var(--e{})", i + 1, i + 1),
            )
        })
        .chain([("--e10".to_string(), String::new())])
        .collect();
    let pairs: Vec<(&str, &str)> = c.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    assert_eq!(res("var(--e0)", &pairs), Err(VarError::TooLong));
    let long = "x".repeat(600);
    assert_eq!(
        res("var(--l) var(--l)", &[("--l", &long)]),
        Err(VarError::TooLong)
    );
    let many = "var(--a)".repeat(MAX_EXPANSIONS + 1);
    assert_eq!(res(&many, &[("--a", "")]), Err(VarError::TooLong));
    assert_eq!(
        res(&"var(--a)".repeat(MAX_EXPANSIONS), &[("--a", "")]).as_deref(),
        Ok("")
    );
}

#[test]
fn lengths_are_pixels_and_the_unit_is_kept_for_the_report() {
    for (text, px, unit) in [
        ("12px", 12.0, Unit::Px),
        ("12PX", 12.0, Unit::Px),
        ("-3.5px", -3.5, Unit::Px),
        ("0", 0.0, Unit::Px),
        ("1.5rem", 24.0, Unit::Rem),
        ("2em", 40.0, Unit::Em),
        ("-.5em", -10.0, Unit::Em),
        ("12pt", 16.0, Unit::Pt),
        ("calc(8 * 1px)", 8.0, Unit::Calc),
        ("calc( 1px * 8 )", 8.0, Unit::Calc),
        ("CALC(8*1px)", 8.0, Unit::Calc),
    ] {
        let l = length(text, 20.0).unwrap_or_else(|| panic!("{text}"));
        assert_eq!((l.px, l.unit), (px, unit), "{text}");
    }
    for text in [
        "",
        "5",
        "auto",
        "50%",
        "1vw",
        "1ch",
        "1e3px",
        "NaNpx",
        "px",
        "calc(1px + 2px)",
        "calc(2 * 2)",
        "calc(8 * 2px)",
        "min(1px, 2px)",
        "1 px",
        "inf",
        "1px 2px",
    ] {
        assert_eq!(length(text, 20.0), None, "{text}");
    }
}

#[test]
fn colours_are_hex_and_what_meldframe_css_cannot_read_is_refused() {
    for (text, hex) in [
        ("#FFF", "#fff"),
        ("#3366FF80", "#3366ff80"),
        ("red", "#ff0000"),
        ("Transparent", "#00000000"),
        ("rgb(255 0 0)", "#ff0000"),
        ("rgba(0, 0, 0, 0.5)", "#00000080"),
        ("hsl(120 100% 25%)", "#008000"),
    ] {
        assert_eq!(colour(text).map(|c| c.hex).as_deref(), Some(hex), "{text}");
    }
    for text in [
        "currentcolor",
        "Canvas",
        "lab(50% 0 0)",
        "color-mix(in srgb, red, blue)",
        "#12",
        "var(--x)",
        "",
    ] {
        assert_eq!(colour(text), None, "{text}");
    }
    assert!(colour("oklch(0.7 0.4 30)").is_some_and(|c| c.clipped));
}

#[test]
fn a_value_is_cut_at_the_space_outside_brackets_and_strings() {
    assert_eq!(words("  1px  solid\n#000 "), ["1px", "solid", "#000"]);
    assert_eq!(
        words("0 0 1px rgb(0 0 0 / 50%) 'a b' \"c\\\" d\""),
        ["0", "0", "1px", "rgb(0 0 0 / 50%)", "'a b'", "\"c\\\" d\""]
    );
    assert_eq!(words(""), Vec::<&str>::new());
    assert_eq!(words("(a b"), ["(a b"]);
    assert!(has_top_level_comma("a, b") && !has_top_level_comma("f(a, b) 'x,y'"));
    assert!(is_wide(" INHERIT ") && is_wide("unset") && !is_wide("none"));
}
