//! Every `Refusal` has a stable code, and `docs/error-codes.md` lists exactly the gate codes.

use std::collections::BTreeSet;

use meldframe_check::ApplyError;
use meldframe_host::Refusal;
use meldframe_module::{Policy, RiskTier};

const DOC: &str = include_str!("../../../../docs/error-codes.md");
const HEADING: &str = "\n## Gate refusal codes\n";

/// One index per variant. No wildcard: a new variant stops this file from compiling until it gets an
/// index here. Then raise `VARIANTS` and add a sample in `samples`; a gate variant also needs a docs row
/// (`the_docs_table_lists_exactly_the_gate_codes`).
fn index(r: &Refusal) -> usize {
    match r {
        Refusal::Forbidden { .. } => 0,
        Refusal::QuotaExceeded { .. } => 1,
        Refusal::Conflict { .. } => 2,
        Refusal::Invalid { .. } => 3,
        Refusal::DependencyRejected { .. } => 4,
        Refusal::Apply(_) => 5,
        Refusal::RateExceeded { .. } => 6,
        Refusal::BatchTooLarge { .. } => 7,
        Refusal::StorageExceeded { .. } => 8,
    }
}

const VARIANTS: usize = 9;

fn samples() -> Vec<Refusal> {
    vec![
        Refusal::Forbidden {
            tier: RiskTier::Delete,
            policy: Policy::Ask,
        },
        Refusal::QuotaExceeded { limit: 1 },
        Refusal::Conflict {
            node_id: "a".into(),
        },
        Refusal::Invalid { reason: "x".into() },
        Refusal::DependencyRejected { key: "k".into() },
        Refusal::Apply(ApplyError::Cycle),
        Refusal::RateExceeded {
            limit: 1,
            retry_after_ms: 1,
        },
        Refusal::BatchTooLarge { limit: 1 },
        Refusal::StorageExceeded { limit: 1 },
    ]
}

/// The codes in the gate table: every row starts with a code in backticks.
fn gate_table() -> Vec<String> {
    let section = DOC
        .split(HEADING)
        .nth(1)
        .expect("the gate table is missing");
    let section = section.split("\n## ").next().unwrap_or(section); // up to the next section
    section
        .lines()
        .filter(|l| l.starts_with('|'))
        .filter(|l| !l.starts_with("| Code ") && !l.starts_with("| ---"))
        .map(|l| {
            l.strip_prefix("| `")
                .and_then(|r| r.split('`').next())
                .unwrap_or_else(|| panic!("malformed row: {l}"))
                .to_string()
        })
        .collect()
}

#[test]
fn the_samples_cover_every_variant() {
    let seen: BTreeSet<usize> = samples().iter().map(index).collect();
    assert_eq!(seen, (0..VARIANTS).collect());
    assert_eq!(samples().len(), VARIANTS);
}

#[test]
fn the_gate_codes_are_stable_and_apply_keeps_its_own() {
    let codes: Vec<&str> = samples().iter().map(Refusal::code).collect();
    assert_eq!(
        codes,
        [
            "forbidden",
            "quota_exceeded",
            "conflict",
            "invalid",
            "dependency_rejected",
            "E_CYCLE",
            "quota_ops_per_minute",
            "batch_too_large",
            "quota_storage"
        ]
    );
}

#[test]
fn no_two_variants_share_a_code() {
    let codes: BTreeSet<&str> = samples().iter().map(Refusal::code).collect();
    assert_eq!(codes.len(), VARIANTS);
}

#[test]
fn an_apply_refusal_carries_the_code_of_its_apply_error() {
    for e in [ApplyError::Cycle, ApplyError::NotFound("a".into())] {
        assert_eq!(Refusal::Apply(e.clone()).code(), e.code());
    }
}

#[test]
fn every_message_is_english_and_not_empty() {
    for r in samples() {
        let m = r.to_string();
        assert!(!m.is_empty() && m.is_ascii(), "{m}");
    }
}

#[test]
fn the_docs_table_lists_exactly_the_gate_codes() {
    let in_code: BTreeSet<String> = samples()
        .iter()
        .filter(|r| !matches!(r, Refusal::Apply(_)))
        .map(|r| r.code().to_string())
        .collect();
    let docs = gate_table();
    let in_docs: BTreeSet<String> = docs.iter().cloned().collect();
    assert_eq!(docs.len(), in_docs.len(), "a code is listed twice");
    assert_eq!(in_docs, in_code);
}
