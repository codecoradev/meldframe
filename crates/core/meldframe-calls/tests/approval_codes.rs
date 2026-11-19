//! E2.4: how the codes of the approval path read in the call log.

use meldframe_calls::{Decision, describe_code};

#[test]
fn a_call_that_waits_for_the_owner_is_an_ask() {
    assert_eq!(
        describe_code(Some("approval_required")),
        (Some(Decision::Ask), "approval_required".into())
    );
}

#[test]
fn a_denied_expired_or_refused_for_limit_approval_is_a_deny() {
    for code in ["approval_denied", "approval_expired", "approval_limit"] {
        assert_eq!(
            describe_code(Some(code)),
            (Some(Decision::Deny), code.into())
        );
    }
}

#[test]
fn the_owners_answer_is_one_entry_with_the_decision_and_no_content() {
    use meldframe_calls::{CallEntry, TOOL_DECIDE_APPROVAL};
    use meldframe_host::Principal;
    use meldframe_module::RiskTier;
    let owner = Principal::new("ws_1", "owner:o");
    let yes = CallEntry::approval_decision(5, &owner, "f1", true);
    let no = CallEntry::approval_decision(6, &owner, "f1", false);
    assert_eq!(
        (
            yes.tool.as_str(),
            yes.risk,
            yes.decision,
            yes.outcome.as_str()
        ),
        (
            TOOL_DECIDE_APPROVAL,
            Some(RiskTier::Delete),
            Some(Decision::Allow),
            "ok"
        )
    );
    assert_eq!(no.decision, Some(Decision::Deny));
    assert_eq!((no.actor.as_str(), no.file.as_str()), ("owner:o", "f1"));
}
