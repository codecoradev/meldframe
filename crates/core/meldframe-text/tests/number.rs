//! `canonical_u32`: the one spelling of a whole number that a path, a key or a store accepts. Four crates kept their own
//! copy and the copies disagreed on `0` and on `01`; this table decides it once.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_text::canonical_u32;

#[test]
fn zero_is_one_digit_and_no_other_number_starts_with_zero() {
    // Zero has exactly one spelling. The review route, the room key and the batch key of a revert took `0`; the
    // checkpoint route refused it (its numbers start at 1), which is a bound of that caller and not a spelling.
    assert_eq!(canonical_u32("0"), Some(0));
    for aliased in ["00", "01", "007", "0000000001"] {
        assert_eq!(canonical_u32(aliased), None, "{aliased:?} aliases a number");
    }
}

#[test]
fn a_number_is_ascii_digits_and_nothing_else() {
    for plain in ["1", "9", "10", "42", "123456789", "4294967295"] {
        assert_eq!(canonical_u32(plain), plain.parse().ok(), "{plain:?}");
    }
    for other in [
        "",
        "+1",
        "-1",
        "-0",
        "+0",
        "1 ",
        " 1",
        "1\n",
        "\t1",
        "1.0",
        "1e3",
        "0x10",
        "1_000",
        "1,000",
        // Digits of other scripts, which `str::parse` also refuses and `char::is_numeric` would let through.
        "\u{0663}",
        "\u{FF11}",
        "1\u{200B}",
    ] {
        assert_eq!(canonical_u32(other), None, "{other:?}");
    }
}

#[test]
fn the_range_is_that_of_a_u32() {
    assert_eq!(canonical_u32("4294967295"), Some(u32::MAX));
    assert_eq!(canonical_u32("4294967296"), None);
    assert_eq!(canonical_u32("99999999999"), None);
    assert_eq!(canonical_u32("18446744073709551616"), None);
}
