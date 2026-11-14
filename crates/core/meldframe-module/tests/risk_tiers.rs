//! Risk tiers and their default policy (ADR-0010, questionnaire section 4).

use meldframe_module::{Policy, RiskTier, default_policy, fallback_policy};

#[test]
fn read_and_change_default_to_auto() {
    assert_eq!(default_policy(RiskTier::Read), Policy::Auto);
    assert_eq!(default_policy(RiskTier::Change), Policy::Auto);
}

#[test]
fn delete_defaults_to_ask() {
    assert_eq!(default_policy(RiskTier::Delete), Policy::Ask);
}

#[test]
fn unreadable_policy_means_reads_only() {
    assert_eq!(fallback_policy(RiskTier::Read), Policy::Auto);
    assert_eq!(fallback_policy(RiskTier::Change), Policy::Deny);
    assert_eq!(fallback_policy(RiskTier::Delete), Policy::Deny);
}

#[test]
fn read_is_never_denied_or_asked_by_any_default() {
    for tier in RiskTier::ALL {
        if tier == RiskTier::Read {
            assert_eq!(default_policy(tier), Policy::Auto);
            assert_eq!(fallback_policy(tier), Policy::Auto);
        }
    }
}

#[test]
fn tiers_have_stable_names() {
    let names: Vec<&str> = RiskTier::ALL.iter().map(|t| t.as_str()).collect();
    assert_eq!(names, ["read", "change", "delete"]);
}
