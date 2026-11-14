//! Risk tiers and the default policy per tier (ADR-0010).

/// How much a call can change user data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RiskTier {
    /// Reads data only.
    Read,
    /// Changes data and can be reverted.
    Change,
    /// Removes data; deleted data goes to the trash first.
    Delete,
}

impl RiskTier {
    /// Every tier, from least to most risky.
    pub const ALL: [RiskTier; 3] = [RiskTier::Read, RiskTier::Change, RiskTier::Delete];

    /// The stable lower-case name of the tier, as used on the wire and in tool text.
    pub fn as_str(self) -> &'static str {
        match self {
            RiskTier::Read => "read",
            RiskTier::Change => "change",
            RiskTier::Delete => "delete",
        }
    }
}

/// What the gate does with a call of a given tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Policy {
    /// Apply at once; review afterwards.
    Auto,
    /// Ask the owner first.
    Ask,
    /// Refuse.
    Deny,
}

/// The default policy of a tier, from the owner questionnaire (section 4): Read is Auto, Change is
/// Auto and Delete is Ask.
///
/// Delete is Ask at the tier level. The gate may relax it to Auto below a size threshold; that
/// threshold depends on the call and lives in the gate, not here. Per-agent overrides are not
/// represented yet.
pub fn default_policy(tier: RiskTier) -> Policy {
    match tier {
        RiskTier::Read | RiskTier::Change => Policy::Auto,
        RiskTier::Delete => Policy::Ask,
    }
}

/// The policy to use when the real policy cannot be read: reads only.
pub fn fallback_policy(tier: RiskTier) -> Policy {
    match tier {
        RiskTier::Read => Policy::Auto,
        RiskTier::Change | RiskTier::Delete => Policy::Deny,
    }
}
