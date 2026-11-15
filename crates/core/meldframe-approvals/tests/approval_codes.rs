//! The approval codes are stable and `docs/error-codes.md` lists exactly them.

use std::collections::BTreeSet;

use meldframe_approvals::ApprovalRefusal;
use meldframe_host::Refusal;
use meldframe_module::{Policy, RiskTier};

const DOC: &str = include_str!("../../../../docs/error-codes.md");
const HEADING: &str = "\n## Approval refusal codes\n";

/// One index per variant, no wildcard: a new variant stops this file from compiling until it gets a sample
/// and a docs row.
fn index(r: &ApprovalRefusal) -> usize {
    match r {
        ApprovalRefusal::Required { .. } => 0,
        ApprovalRefusal::Denied { .. } => 1,
        ApprovalRefusal::Expired { .. } => 2,
        ApprovalRefusal::Refused(_) => 3,
    }
}

fn samples() -> Vec<ApprovalRefusal> {
    let id = || "apr_1".to_string();
    vec![
        ApprovalRefusal::Required { id: id() },
        ApprovalRefusal::Denied { id: id() },
        ApprovalRefusal::Expired { id: id() },
        ApprovalRefusal::Refused(Refusal::Forbidden {
            tier: RiskTier::Delete,
            policy: Policy::Deny,
        }),
    ]
}

/// The codes in the approval table: every row starts with a code in backticks, up to the next heading.
fn table() -> Vec<String> {
    let section = DOC
        .split(HEADING)
        .nth(1)
        .expect("the approval table is missing");
    let section = section.split("\n## ").next().unwrap();
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
    assert_eq!(seen, (0..4).collect());
}

#[test]
fn the_codes_are_stable_and_a_gate_refusal_keeps_its_own() {
    let codes: Vec<&str> = samples().iter().map(ApprovalRefusal::code).collect();
    assert_eq!(
        codes,
        [
            "approval_required",
            "approval_denied",
            "approval_expired",
            "forbidden"
        ]
    );
}

#[test]
fn every_message_is_english_and_not_empty() {
    for r in samples() {
        let m = r.to_string();
        assert!(!m.is_empty() && m.is_ascii(), "{m}");
    }
}

#[test]
fn the_docs_table_lists_exactly_the_approval_codes() {
    let in_code: BTreeSet<String> = samples()
        .iter()
        .filter(|r| !matches!(r, ApprovalRefusal::Refused(_)))
        .map(|r| r.code().to_string())
        .collect();
    let docs = table();
    let in_docs: BTreeSet<String> = docs.iter().cloned().collect();
    assert_eq!(docs.len(), in_docs.len(), "a code is listed twice");
    assert_eq!(in_docs, in_code);
}
