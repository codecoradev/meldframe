//! The length reader: `px`, `rem` and a bare number, read as pixels; anything else is not a length here.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_css::length::{Length, REM_PX, length};

fn px(value: f64) -> Option<Length> {
    Some(Length {
        value,
        from_rem: false,
    })
}

fn rem(value: f64) -> Option<Length> {
    Some(Length {
        value,
        from_rem: true,
    })
}

#[test]
fn px_and_plain_numbers_are_exact_and_rem_is_sixteen_times_and_marked() {
    assert_eq!(length("12px"), px(12.0));
    assert_eq!(length("0.5"), px(0.5));
    assert_eq!(length("-4px"), px(-4.0));
    assert_eq!(length("0"), px(0.0));
    assert_eq!(length("1.5rem"), rem(24.0));
    assert_eq!(length("2REM"), rem(32.0));
    assert_eq!(REM_PX, 16.0);
}

#[test]
fn text_that_is_not_a_finite_number_with_px_or_rem_is_not_a_length() {
    for s in [
        "",
        "px",
        "rem",
        "1em",
        "50%",
        "10vh",
        "2s",
        "NaN",
        "inf",
        "1e3",
        "1e999px",
        "1 2",
        "--1",
        "1px 2px",
        "calc(1px)",
    ] {
        assert_eq!(length(s), None, "{s:?}");
    }
}

#[test]
fn white_space_between_the_number_and_the_unit_or_before_the_number_is_read_and_a_sign_is_kept() {
    assert_eq!(length("12 px"), px(12.0));
    assert_eq!(length(" 12px"), px(12.0));
    assert_eq!(length("1.5 rem"), rem(24.0));
    assert_eq!(length(" 1.5rem"), rem(24.0));
    assert_eq!(length("+5px"), px(5.0));
    assert_eq!(length("-0.5rem"), rem(-8.0));
    assert_eq!(length("12 PX"), px(12.0));
}

#[test]
fn white_space_after_the_unit_is_not_read_so_the_caller_trims_first() {
    // The unit has to end the text: a caller with untrimmed text trims it, as the variables import does.
    for s in ["  2REM ", "12px ", " 12px ", "2rem\n"] {
        assert_eq!(length(s), None, "{s:?}");
    }
    assert_eq!(length("  2REM ".trim()), rem(32.0));
}
