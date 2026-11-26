//! The shapes of the ids the model names things with: an asset id, and the plain lower-case hex it is made of. The
//! rule is written once here; the checks, the asset store, the export and the importer call it and keep their own
//! errors.

use meldframe_protocol::{ASSET_ID_LEN, is_asset_id, is_lower_hex};

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
