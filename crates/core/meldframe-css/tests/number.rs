//! The one CSS number reader: digits, a point and a sign, with or without an exponent. Finite, and nothing else.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_css::number::{number, number_exp, starts_like_number};

#[test]
fn a_plain_number_has_digits_a_point_and_a_sign() {
    assert_eq!(number("12"), Some(12.0));
    assert_eq!(number("0.5"), Some(0.5));
    assert_eq!(number(".5"), Some(0.5));
    assert_eq!(number("-4"), Some(-4.0));
    assert_eq!(number("+3.25"), Some(3.25));
}

#[test]
fn the_text_is_trimmed() {
    assert_eq!(number("  7 "), Some(7.0));
    assert_eq!(number_exp(" 1e2\t"), Some(100.0));
}

#[test]
fn an_empty_text_or_a_stray_character_is_no_number() {
    for s in ["", "  ", "a", "1px", "1 2", "1_0", "0x10", "1%", "--"] {
        assert_eq!(number(s), None, "{s:?}");
        assert_eq!(number_exp(s), None, "{s:?}");
    }
}

#[test]
fn a_word_that_rust_would_parse_is_no_number() {
    for s in ["NaN", "nan", "inf", "-inf", "infinity", "Infinity"] {
        assert_eq!(number(s), None, "{s:?}");
        assert_eq!(number_exp(s), None, "{s:?}");
    }
}

#[test]
fn the_plain_reader_refuses_an_exponent_and_the_other_reads_it() {
    assert_eq!(number("1e2"), None);
    assert_eq!(number("1E2"), None);
    assert_eq!(number_exp("1e2"), Some(100.0));
    assert_eq!(number_exp("1E-2"), Some(0.01));
    assert_eq!(number_exp("-2.5e+1"), Some(-25.0));
}

#[test]
fn a_number_that_does_not_parse_is_none() {
    for s in ["1.2.3", "+-1", "1-", "e", "1e", "e5"] {
        assert_eq!(number(s), None, "{s:?}");
        assert_eq!(number_exp(s), None, "{s:?}");
    }
}

#[test]
fn a_number_that_overflows_is_none_so_the_result_is_always_finite() {
    let big = format!("1{}", "0".repeat(400));
    assert_eq!(number(&big), None);
    assert_eq!(number_exp("1e999"), None);
    assert_eq!(number_exp("-1e999"), None);
}

#[test]
fn a_word_starts_like_a_number_on_a_digit_a_point_or_a_sign() {
    for w in ["10px", "-3", ".5", "+1%", "0", "9deg"] {
        assert!(starts_like_number(w), "{w:?}");
    }
    for w in ["", "px", "red", "e5", " 1", "#fff", "calc(1px)"] {
        assert!(!starts_like_number(w), "{w:?}");
    }
}
