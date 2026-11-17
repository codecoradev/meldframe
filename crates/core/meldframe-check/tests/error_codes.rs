//! Every `ApplyError` has a stable machine code, and `docs/error-codes.md` lists exactly those codes.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use meldframe_check::{ApplyError, LoadError};

const DOC: &str = include_str!("../../../../docs/error-codes.md");

/// One value of every variant. The match below has no wildcard: a new variant stops this file from
/// compiling until it gets an index here, a sample in `samples`, and a row in the docs table.
fn index(e: &ApplyError) -> usize {
    match e {
        ApplyError::NotFound(_) => 0,
        ApplyError::DuplicateId(_) => 1,
        ApplyError::ParentNotFound(_) => 2,
        ApplyError::ParentNotFrame(_) => 3,
        ApplyError::Cycle => 4,
        ApplyError::TooManyNodes(_) => 5,
        ApplyError::TooDeep(_) => 6,
        ApplyError::InvalidId(_) => 7,
        ApplyError::NotFinite(_) => 8,
        ApplyError::OutOfRange { .. } => 9,
        ApplyError::TooLong { .. } => 10,
        ApplyError::InvalidColor { .. } => 11,
        ApplyError::InvalidPos(_) => 12,
        ApplyError::PosTooLong { .. } => 13,
        ApplyError::BadReorder(_) => 14,
        ApplyError::TooMany { .. } => 15,
        ApplyError::InvalidFontWeight(_) => 16,
        ApplyError::InvalidAssetId => 17,
        ApplyError::ImageOnText => 18,
        ApplyError::InvalidGradient(_) => 19,
        ApplyError::GradientOnText => 20,
    }
}

const VARIANTS: usize = 21;

/// The same for `LoadError`. Its `Node` variant carries an `ApplyError` and has that error's code, so it
/// adds no row of its own; every other variant needs one.
fn load_index(e: &LoadError) -> usize {
    match e {
        LoadError::Node { .. } => 0,
        LoadError::MissingPos(_) => 1,
        LoadError::DuplicatePos { .. } => 2,
    }
}

fn load_samples() -> Vec<LoadError> {
    vec![
        LoadError::Node {
            node: Some("a".into()),
            error: ApplyError::Cycle,
        },
        LoadError::MissingPos("a".into()),
        LoadError::DuplicatePos {
            parent: None,
            pos: "a0".into(),
        },
    ]
}

fn samples() -> Vec<ApplyError> {
    vec![
        ApplyError::NotFound("a".into()),
        ApplyError::DuplicateId("a".into()),
        ApplyError::ParentNotFound("a".into()),
        ApplyError::ParentNotFrame("a".into()),
        ApplyError::Cycle,
        ApplyError::TooManyNodes(1),
        ApplyError::TooDeep(1),
        ApplyError::InvalidId("a".into()),
        ApplyError::NotFinite("x"),
        ApplyError::OutOfRange {
            field: "x",
            value: 1.0,
        },
        ApplyError::TooLong { field: "x", max: 1 },
        ApplyError::InvalidColor { field: "x" },
        ApplyError::InvalidPos("a".into()),
        ApplyError::PosTooLong { max: 1 },
        ApplyError::BadReorder("a".into()),
        ApplyError::TooMany {
            field: "shadows",
            max: 1,
        },
        ApplyError::InvalidFontWeight(450),
        ApplyError::InvalidAssetId,
        ApplyError::ImageOnText,
        ApplyError::InvalidGradient("gradient.stops needs at least 2 stops"),
        ApplyError::GradientOnText,
    ]
}

/// The codes in the docs table. Every table row must start with a code in backticks: a row that does
/// not (an extra or malformed one) fails the test instead of being skipped.
fn documented() -> Vec<String> {
    let mut codes = Vec::new();
    // The gate table below the heading belongs to `meldframe-host`, which tests it.
    let apply_part = DOC.split("\n## Gate refusal codes\n").next().unwrap();
    for l in apply_part.lines().filter(|l| l.starts_with('|')) {
        if l.starts_with("| Code ") || l.starts_with("| ---") {
            continue;
        }
        let code = l
            .strip_prefix("| `")
            .and_then(|r| r.split('`').next())
            .unwrap_or_else(|| panic!("malformed row in docs/error-codes.md: {l}"));
        assert!(code.starts_with("E_"), "row without an E_ code: {l}");
        codes.push(code.to_string());
    }
    codes
}

#[test]
fn the_samples_cover_every_variant() {
    let seen: BTreeSet<usize> = samples().iter().map(index).collect();
    assert_eq!(seen, (0..VARIANTS).collect());
    assert_eq!(samples().len(), VARIANTS);
}

#[test]
fn every_variant_has_a_screaming_snake_code_that_starts_with_e() {
    for e in samples() {
        let c = e.code();
        assert!(c.starts_with("E_"), "{c}");
        assert!(
            c.bytes().all(|b| b.is_ascii_uppercase() || b == b'_'),
            "{c}"
        );
    }
}

#[test]
fn no_two_variants_share_a_code() {
    let codes: BTreeSet<&str> = samples().iter().map(ApplyError::code).collect();
    assert_eq!(codes.len(), VARIANTS);
}

#[test]
fn the_code_does_not_depend_on_the_details() {
    assert_eq!(
        ApplyError::NotFound("a".into()).code(),
        ApplyError::NotFound("b".into()).code()
    );
    assert_eq!(
        ApplyError::ParentNotFound("a".into()).code(),
        "E_PARENT_NOT_FOUND"
    );
}

#[test]
fn the_docs_table_lists_exactly_the_codes_in_the_code() {
    let in_code: BTreeSet<String> = samples()
        .iter()
        .map(|e| e.code().to_string())
        .chain(
            load_samples()
                .iter()
                .filter(|e| !matches!(e, LoadError::Node { .. }))
                .map(|e| e.code().to_string()),
        )
        .collect();
    let docs = documented();
    let in_docs: BTreeSet<String> = docs.iter().cloned().collect();
    assert_eq!(docs.len(), in_docs.len(), "a code is listed twice");
    assert_eq!(in_docs, in_code);
}

#[test]
fn the_load_samples_cover_every_variant() {
    let seen: BTreeSet<usize> = load_samples().iter().map(load_index).collect();
    assert_eq!(seen, (0..3).collect());
}

#[test]
fn a_load_error_that_wraps_an_apply_error_has_its_code() {
    assert_eq!(load_samples()[0].code(), "E_CYCLE");
    assert_eq!(load_samples()[1].code(), "E_MISSING_POS");
    assert_eq!(load_samples()[2].code(), "E_DUPLICATE_POS");
}
