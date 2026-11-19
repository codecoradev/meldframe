//! The codes of `VarError`: one per variant, none shared, the same as `VAR_ERROR_CODES`, and every one has a row
//! in `docs/error-codes.md` (`meldframe-check` tests that the docs table has no other row).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_protocol::VarType;
use meldframe_variables::{VAR_ERROR_CODES, VarError};
use std::collections::BTreeSet;

const DOC: &str = include_str!("../../../../docs/error-codes.md");

/// One index per variant. The match has no wildcard: a new variant stops this file from compiling until it gets
/// an index here and a sample below.
fn index(e: &VarError) -> usize {
    match e {
        VarError::VariableNotFound(_) => 0,
        VarError::CollectionNotFound(_) => 1,
        VarError::ModeNotFound(_) => 2,
        VarError::InvalidName(_) => 3,
        VarError::DuplicateName(_) => 4,
        VarError::TypeMismatch { .. } => 5,
        VarError::AliasCycle(_) => 6,
        VarError::AliasTooDeep(_) => 7,
        VarError::VariableInUse { .. } => 8,
        VarError::CollectionNotEmpty(_) => 9,
        VarError::DefaultMode(_) => 10,
        VarError::IncompleteValues(_) => 11,
        VarError::InvalidModes(_) => 12,
        VarError::NoVariables => 13,
        VarError::Common { .. } => 14,
    }
}

fn samples() -> Vec<VarError> {
    vec![
        VarError::VariableNotFound("a".into()),
        VarError::CollectionNotFound("a".into()),
        VarError::ModeNotFound("a".into()),
        VarError::InvalidName("A".into()),
        VarError::DuplicateName("a".into()),
        VarError::TypeMismatch {
            field: "fill".into(),
            expected: VarType::Color,
            got: VarType::Number,
        },
        VarError::AliasCycle("a".into()),
        VarError::AliasTooDeep(16),
        VarError::VariableInUse {
            id: "a".into(),
            by: vec!["n".into()],
        },
        VarError::CollectionNotEmpty("a".into()),
        VarError::DefaultMode("a".into()),
        VarError::IncompleteValues("a".into()),
        VarError::InvalidModes("a collection needs at least one mode"),
        VarError::NoVariables,
        VarError::Common {
            code: "E_INVALID_ID",
            message: "invalid id".into(),
        },
    ]
}

#[test]
fn the_samples_cover_every_variant() {
    let seen: BTreeSet<usize> = samples().iter().map(index).collect();
    assert_eq!(seen, (0..15).collect());
}

#[test]
fn the_codes_of_the_variants_are_the_list_and_are_documented() {
    let own: Vec<&str> = samples()
        .iter()
        .filter(|e| !matches!(e, VarError::Common { .. }))
        .map(VarError::code)
        .collect();
    assert_eq!(
        own, VAR_ERROR_CODES,
        "VAR_ERROR_CODES lists the codes in the order of the variants"
    );
    assert_eq!(
        own.iter().collect::<BTreeSet<_>>().len(),
        own.len(),
        "a code is shared"
    );
    for code in own {
        assert!(
            DOC.contains(&format!("| `{code}` |")),
            "{code} has no row in docs/error-codes.md"
        );
    }
}
