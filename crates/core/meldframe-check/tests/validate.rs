//! Validation of untrusted node values: numbers, strings, ids, colours and `pos` are checked
//! against `Limits`, and every refusal names the field.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_check::{ApplyError, Limits, check_node, check_patch, check_pos};
use meldframe_protocol::{Node, NodePatch};

fn node(json: serde_json::Value) -> Node {
    serde_json::from_value(json).unwrap()
}

fn rect(id: &str) -> Node {
    node(serde_json::json!({"id": id, "type": "rect"}))
}

fn patch(f: impl FnOnce(&mut NodePatch)) -> NodePatch {
    let mut p = NodePatch::default();
    f(&mut p);
    p
}

#[test]
fn accepts_a_default_node_and_an_empty_patch() {
    let l = Limits::default();
    assert_eq!(check_node(&rect("a"), &l), Ok(()));
    assert_eq!(check_patch(&NodePatch::default(), &l), Ok(()));
}

#[test]
fn rejects_out_of_range_numbers_in_a_patch() {
    let l = Limits::default();
    let bad = [
        patch(|p| p.w = Some(1.0e9)),
        patch(|p| p.w = Some(-1.0)),
        patch(|p| p.x = Some(2.0e6)),
        patch(|p| p.gap = Some(100_001.0)),
        patch(|p| p.font_size = Some(0.0)),
    ];
    for (i, p) in bad.iter().enumerate() {
        assert!(
            matches!(check_patch(p, &l), Err(ApplyError::OutOfRange { .. })),
            "case {i}"
        );
    }
    let e = check_patch(&patch(|p| p.w = Some(1.0e9)), &l).unwrap_err();
    assert_eq!(
        e,
        ApplyError::OutOfRange {
            field: "w",
            value: 1.0e9
        }
    );
    assert!(e.to_string().contains('w'), "{e}");
}

#[test]
fn rejects_non_finite_numbers() {
    let l = Limits::default();
    let nan = check_patch(&patch(|p| p.x = Some(f64::NAN)), &l);
    assert_eq!(nan, Err(ApplyError::NotFinite("x")));
    let inf = check_patch(&patch(|p| p.h = Some(f64::INFINITY)), &l);
    assert_eq!(inf, Err(ApplyError::NotFinite("h")));
    let mut n = rect("a");
    n.radius = f64::NEG_INFINITY.into();
    assert_eq!(check_node(&n, &l), Err(ApplyError::NotFinite("radius")));
}

#[test]
fn rejects_long_strings() {
    let l = Limits::default();
    let long_text = patch(|p| p.text = Some("x".repeat(l.max_text + 1)));
    assert_eq!(
        check_patch(&long_text, &l),
        Err(ApplyError::TooLong {
            field: "text",
            max: 20_000
        })
    );
    let long_name = patch(|p| p.name = Some("x".repeat(l.max_name + 1)));
    assert_eq!(
        check_patch(&long_name, &l),
        Err(ApplyError::TooLong {
            field: "name",
            max: 256
        })
    );
    // The limit counts characters, not bytes.
    let multibyte = patch(|p| p.name = Some("é".repeat(l.max_name)));
    assert_eq!(check_patch(&multibyte, &l), Ok(()));
}

#[test]
fn rejects_malformed_ids() {
    let l = Limits::default();
    for bad in ["", "bad id!", "a/b", "é", &"x".repeat(l.max_id + 1)] {
        assert!(
            matches!(check_node(&rect(bad), &l), Err(ApplyError::InvalidId(_))),
            "{bad:?}"
        );
    }
    for ok in ["a", "node_1", "a-b", "scope:item.2", &"x".repeat(l.max_id)] {
        assert_eq!(check_node(&rect(ok), &l), Ok(()), "{ok:?}");
    }
}

#[test]
fn rejects_non_hex_colours_and_names_the_field() {
    let l = Limits::default();
    let bad_colours = [
        "linear-gradient(red, blue)",
        "red",
        "rgb(1,2,3)",
        "#12",
        "#12345",
        "#1234567",
        "#12345678a",
        "#gggggg",
        "",
        " #fff",
    ];
    for bad in bad_colours {
        let mut n = rect("a");
        n.fill = bad.into();
        let e = check_node(&n, &l).expect_err(bad);
        assert_eq!(e, ApplyError::InvalidColor { field: "fill" }, "{bad:?}");
        assert!(e.to_string().contains("fill"), "{e}");
        let mut n = rect("a");
        n.fill2 = bad.into();
        assert_eq!(
            check_node(&n, &l),
            Err(ApplyError::InvalidColor { field: "fill2" })
        );
        let p = patch(|p| p.fill = Some(bad.into()));
        assert_eq!(
            check_patch(&p, &l),
            Err(ApplyError::InvalidColor { field: "fill" })
        );
        let p = patch(|p| p.fill2 = Some(bad.into()));
        assert_eq!(
            check_patch(&p, &l),
            Err(ApplyError::InvalidColor { field: "fill2" })
        );
    }
}

#[test]
fn accepts_all_four_hex_forms_in_either_case() {
    let l = Limits::default();
    for ok in [
        "#fff",
        "#FFF",
        "#ffff",
        "#FfFa",
        "#a1b2c3",
        "#A1B2C3",
        "#00000000",
        "#a1b2c3d4",
    ] {
        let mut n = rect("a");
        n.fill = ok.into();
        assert_eq!(check_node(&n, &l), Ok(()), "{ok}");
        assert_eq!(
            check_patch(&patch(|p| p.fill2 = Some(ok.into())), &l),
            Ok(()),
            "{ok}"
        );
    }
}

#[test]
fn pos_must_be_a_valid_key_within_the_length_limit() {
    let l = Limits::default();
    assert_eq!(check_pos("a0", &l), Ok(()));
    assert_eq!(check_pos("a0V", &l), Ok(()));
    let bad_keys = [
        "!",
        "a0 ",
        "é",
        "a10",
        "a",
        "A00000000000000000000000000",
        "a0\u{0}",
    ];
    for bad in bad_keys {
        assert!(
            matches!(check_pos(bad, &l), Err(ApplyError::InvalidPos(_))),
            "{bad:?}"
        );
    }
    let long = format!("a0{}", "V".repeat(l.max_pos));
    assert_eq!(
        check_pos(&long, &l),
        Err(ApplyError::PosTooLong { max: 64 })
    );
    // A node without a pos (an old document) is valid; one with a bad pos is not.
    assert_eq!(check_node(&rect("a"), &l), Ok(()));
    let mut n = rect("a");
    n.pos = "a10".into();
    assert!(matches!(check_node(&n, &l), Err(ApplyError::InvalidPos(_))));
}

#[test]
fn a_refused_pos_is_cut_to_32_characters_in_the_error() {
    let l = Limits {
        max_pos: 1000,
        ..Limits::default()
    };
    let e = check_pos(&"!".repeat(200), &l).unwrap_err();
    assert_eq!(e, ApplyError::InvalidPos("!".repeat(32)));
}
