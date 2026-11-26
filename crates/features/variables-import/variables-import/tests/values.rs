//! What a value is: a number, an alias, or something the model cannot hold, and the reason.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use variables_import::value::{Conversion, MAX_VALUE, Val, classify};

fn number(css: &str) -> (f64, Option<Conversion>) {
    match classify("n", css) {
        Val::Number { value, how } => (value, how),
        other => panic!("{css} is not read as a number: {other:?}"),
    }
}

fn skipped(name: &str, css: &str) -> &'static str {
    match classify(name, css) {
        Val::Skip(code, _) => code,
        other => panic!("{css} is not skipped: {other:?}"),
    }
}

#[test]
fn px_and_plain_numbers_are_exact_and_rem_is_sixteen_times_and_a_conversion() {
    assert_eq!(number("12px"), (12.0, None));
    assert_eq!(number("0.5"), (0.5, None));
    assert_eq!(number("-4px"), (-4.0, None));
    assert_eq!(number("0"), (0.0, None));
    assert_eq!(number("1.5rem"), (24.0, Some(Conversion::Rem)));
    assert_eq!(number("  2REM "), (32.0, Some(Conversion::Rem)));
}

#[test]
fn a_pure_var_is_an_alias_and_a_fallback_is_marked() {
    assert_eq!(
        classify("a", "var(--b)"),
        Val::Alias {
            target: "--b".into(),
            fallback: false
        }
    );
    assert_eq!(
        classify("a", " var( --b ) "),
        Val::Alias {
            target: "--b".into(),
            fallback: false
        }
    );
    assert_eq!(
        classify("a", "var(--b, 4px)"),
        Val::Alias {
            target: "--b".into(),
            fallback: true
        }
    );
    assert_eq!(
        classify("a", "var(--b, var(--c))"),
        Val::Alias {
            target: "--b".into(),
            fallback: true
        }
    );
    for bad in ["var(b)", "var(--)", "var()", "var(--a b)"] {
        assert!(matches!(classify("a", bad), Val::Skip(..)), "{bad}");
    }
}

#[test]
fn computed_values_are_skipped_and_never_evaluated() {
    for v in [
        "calc(1rem * 2)",
        "clamp(1rem, 2vw, 3rem)",
        "min(1px, 2px)",
        "max(1px, 2px)",
        "env(safe-area-inset-top)",
        "calc(var(--x) + 1px)",
        "0 0 0 1px var(--border)",
    ] {
        assert_eq!(skipped("a", v), "computed_value", "{v}");
    }
}

#[test]
fn url_is_skipped_and_never_followed() {
    for v in [
        "url(\"https://x.example/i.png\")",
        "url(file:///etc/passwd)",
        "image-set(\"a.png\" 1x)",
    ] {
        assert_eq!(skipped("bg", v), "url_skipped", "{v}");
    }
}

#[test]
fn strings_shadows_fonts_and_other_units_have_no_type_and_are_skipped() {
    for v in [
        "\"Inter\", sans-serif",
        "0 1px 2px #000",
        "0 1px 2px 0 rgb(0 0 0 / 0.05)",
        "cubic-bezier(0.2, 0, 0, 1)",
        "2s",
        "50%",
        "10vh",
        "1em",
        "inherit",
    ] {
        assert_eq!(skipped("a", v), "unsupported_type", "{v}");
    }
    assert_eq!(skipped("a", ""), "empty_value");
    assert_eq!(skipped("a", &"9".repeat(MAX_VALUE + 1)), "value_too_long");
}
