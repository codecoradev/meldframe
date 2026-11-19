//! The in-memory store against the shared contract, one test per case, and the module's registration.

use design_review::{DesignReviewModule, MemoryReviews, TOOL, contract};
use meldframe_module::{Policy, Registry, RiskTier, default_policy};

macro_rules! case {
    ($($name:ident),* $(,)?) => {$(
        #[test]
        fn $name() {
            contract::$name(&MemoryReviews::default());
        }
    )*};
}

case!(
    numbers_start_at_one_and_never_repeat_per_file,
    files_and_workspaces_are_separate_lists,
    a_text_is_checked_before_anything_is_written,
    a_file_holds_a_limited_number_of_items_and_a_refused_add_writes_nothing,
    a_batch_is_linked_once_and_the_list_is_limited,
    an_item_is_marked_addressed_and_open_again,
    an_item_of_another_workspace_or_file_is_not_found,
);

#[test]
fn the_module_registers_one_change_tier_tool() {
    let mut reg = Registry::new();
    reg.register(&DesignReviewModule).unwrap();
    let tool = reg.agent_tool(TOOL).expect("registered");
    assert_eq!(tool.tier, RiskTier::Change);
    assert_eq!(reg.policy_for(TOOL), Some(Policy::Auto));
    assert_eq!(Some(default_policy(RiskTier::Change)), reg.policy_for(TOOL));
}

#[test]
fn every_code_is_stable_lower_case() {
    for c in design_review::CODES {
        assert!(
            c.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
            "{c}"
        );
    }
}
