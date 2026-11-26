//! The shapes of the ids the model names things with: an asset id, the plain lower-case hex it is made of, and the id
//! of a node, a collection, a mode or a variable. The rule is written once here; the checks, the asset store, the
//! export and the importer call it and keep their own errors.

use meldframe_protocol::{ASSET_ID_LEN, is_asset_id, is_lower_hex, valid_id};

#[test]
fn an_id_is_letters_digits_and_four_marks_up_to_the_given_length() {
    for ok in [
        "a",
        "A",
        "0",
        "node_1",
        "a-b",
        "scope:item.2",
        "x".repeat(64).as_str(),
    ] {
        assert!(valid_id(ok, 64), "{ok}");
    }
    for bad in [
        "",
        " ",
        "bad id!",
        "a/b",
        "a\\b",
        "a\nb",
        "a\tb",
        "a\u{7f}b",
        "a\0b",
        "a+b",
        "a,b",
        "a;b",
        "#a",
        "\u{e9}",
        "a\u{200b}",
        "\u{ff21}",
        "\u{661}",
    ] {
        assert!(!valid_id(bad, 64), "{bad:?}");
    }
}

#[test]
fn the_cap_on_the_length_of_an_id_is_the_callers() {
    assert!(valid_id("abc", 3));
    assert!(!valid_id("abcd", 3));
    assert!(valid_id(&"x".repeat(64), 64));
    assert!(!valid_id(&"x".repeat(65), 64));
    assert!(valid_id(&"x".repeat(128), 128));
    // An id of any size is refused by the cap, and one with a multi-byte character is refused whatever the cap.
    assert!(!valid_id(&"x".repeat(1 << 20), 64));
    assert!(!valid_id("\u{e9}", 64) && !valid_id("\u{e9}", 1) && !valid_id("\u{e9}", 2));
    // A cap of 0 holds no id, and the empty text is no id under any cap.
    assert!(!valid_id("x", 0));
    assert!(!valid_id("", 64));
}

#[test]
fn an_asset_id_is_64_lower_case_hex_digits() {
    assert_eq!(ASSET_ID_LEN, 64);
    for ok in ["0".repeat(64), "f".repeat(64), "0123456789abcdef".repeat(4)] {
        assert!(is_asset_id(&ok), "{ok}");
    }
}

#[test]
fn an_asset_id_of_another_length_case_or_alphabet_is_refused() {
    let id = "0123456789abcdef".repeat(4);
    for bad in [
        String::new(),
        id[1..].to_string(),
        format!("{id}0"),
        id.to_uppercase(),
        format!("{}A", &id[1..]),
        format!("{}g", &id[1..]),
        format!(" {}", &id[1..]),
        format!("{}\n", &id[1..]),
        format!("{}/", &id[1..]),
        // 64 bytes, but not 64 hex digits: a two-byte letter in place of two digits.
        format!("{}\u{e9}", &id[2..]),
    ] {
        assert!(!is_asset_id(&bad), "{bad:?}");
    }
}

#[test]
fn lower_case_hex_is_not_empty_and_has_no_upper_case_or_sign() {
    for ok in ["0", "a", "09af", "deadbeef"] {
        assert!(is_lower_hex(ok), "{ok}");
    }
    for bad in ["", "A", "0xff", "-1", "+1", "1 ", "g", "\u{661}"] {
        assert!(!is_lower_hex(bad), "{bad:?}");
    }
}
