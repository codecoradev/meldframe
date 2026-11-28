//! The tool check of a write call: a known tool with an `Auto` policy, and no op above the tier of the tool.

use meldframe_host::{Op, Refusal};
use meldframe_module::{Policy, Registry, RiskTier};
use meldframe_rooms::TreeFacts;

/// The tool is known and its policy is `Auto`; returns its tier. A tool that fails is refused with the highest
/// tier among `ops` (the tool's own tier when the tool is known).
fn tool_tier(
    registry: &Registry,
    tool: &str,
    highest: Option<RiskTier>,
) -> Result<RiskTier, Refusal> {
    let Some(tool_tier) = registry.agent_tool(tool).map(|t| t.tier) else {
        return Err(forbidden(highest.unwrap_or(RiskTier::Read), Policy::Deny));
    };
    match registry.policy_for(tool) {
        Some(Policy::Auto) => Ok(tool_tier),
        Some(policy) => Err(forbidden(tool_tier, policy)),
        None => Err(forbidden(tool_tier, Policy::Deny)),
    }
}

fn highest_tier<'a>(ops: &'a [Op], tier: impl Fn(&'a Op) -> RiskTier) -> Option<RiskTier> {
    ops.iter().map(tier).max_by_key(|t| rank(*t))
}

/// Before any room is opened: known tool, `Auto` policy. Without the tree a delete cannot be told small,
/// so the cap on op tiers waits for [`check_tool`].
pub(crate) fn known_auto_tool(registry: &Registry, tool: &str, ops: &[Op]) -> Result<(), Refusal> {
    tool_tier(registry, tool, highest_tier(ops, Op::tier)).map(|_| ())
}

/// The whole tool check: as above, and no op above the tool's tier, with the tree to tell small deletes. A Delete op
/// of a Change tool is let on to the gate when it may ask (`ask`: the only op of the call, and the node exists): the gate says Ask for it, and
/// the approval path (E2.4) then asks the owner; any other op above the tier is still denied here.
pub(crate) fn check_tool(
    registry: &Registry,
    tool: &str,
    ops: &[Op],
    facts: &TreeFacts<'_>,
    ask: bool,
) -> Result<(), Refusal> {
    let highest = highest_tier(ops, |op| op.tier_with(facts));
    let tier = tool_tier(registry, tool, highest)?;
    match highest.filter(|t| rank(*t) > rank(tier)) {
        Some(RiskTier::Delete) if ask && tier == RiskTier::Change => Ok(()),
        Some(op_tier) => Err(forbidden(op_tier, Policy::Deny)),
        None => Ok(()),
    }
}

fn rank(tier: RiskTier) -> u8 {
    match tier {
        RiskTier::Read => 0,
        RiskTier::Change => 1,
        RiskTier::Delete => 2,
    }
}

fn forbidden(tier: RiskTier, policy: Policy) -> Refusal {
    Refusal::Forbidden { tier, policy }
}
