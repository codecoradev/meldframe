//! The permit: proof that the gate admitted one op.

use meldframe_module::RiskTier;

/// Proof that the gate admitted one op for one actor. Only the gate can create it: the fields and
/// the constructor are private to the `gate` module, so no other code, in this crate or outside it,
/// can build one. It is not `Clone`, and the room takes it by value.
#[derive(Debug)]
pub struct Permit {
    actor: String,
    workspace: String,
    key: String,
    tier: RiskTier,
}

impl Permit {
    pub(super) fn new(actor: &str, workspace: &str, key: &str, tier: RiskTier) -> Self {
        Permit {
            actor: actor.into(),
            workspace: workspace.into(),
            key: key.into(),
            tier,
        }
    }

    /// Who the op is attributed to.
    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// The workspace the principal belongs to.
    pub fn workspace(&self) -> &str {
        &self.workspace
    }

    /// The idempotency key of the admitted op.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The risk tier the gate decided the policy for.
    pub fn tier(&self) -> RiskTier {
        self.tier
    }
}
