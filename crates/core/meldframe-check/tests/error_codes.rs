//! Every `ApplyError` has a stable machine code, and `docs/error-codes.md` lists exactly those codes.

use std::collections::BTreeSet;

use meldframe_check::ApplyError;

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
    }
}

const VARIANTS: usize = 19;

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
    ]
}

/// The codes in the docs table. Every table row must start with a code in backticks: a row that does
/// not (an extra or malformed one) fails the test instead of being skipped.
fn documented() -> Vec<String> {
    let mut codes = Vec::new();
    for l in DOC.lines().filter(|l| l.starts_with('|')) {
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
    let in_code: BTreeSet<String> = samples().iter().map(|e| e.code().to_string()).collect();
    let docs = documented();
    let in_docs: BTreeSet<String> = docs.iter().cloned().collect();
    assert_eq!(docs.len(), in_docs.len(), "a code is listed twice");
    assert_eq!(in_docs, in_code);
}
