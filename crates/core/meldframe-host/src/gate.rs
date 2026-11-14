//! The admission gate (ADR-0010): authorisation by tier policy, quota and shape checks for every op,
//! then a [`Permit`] per admitted op.

mod permit;

pub use permit::Permit;

use crate::{Op, Refusal};
use meldframe_module::{Policy, RiskTier, default_policy, fallback_policy};
use meldframe_protocol::MAX_OP_DEPS;
use std::collections::{HashMap, HashSet};

/// Who submits: the actor and the workspace it acts in. Workspace identity comes from here, never from
/// the op (ADR-0006).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    pub workspace: String,
    pub actor: String,
}

impl Principal {
    pub fn new(workspace: impl Into<String>, actor: impl Into<String>) -> Self {
        Principal {
            workspace: workspace.into(),
            actor: actor.into(),
        }
    }
}

/// One admitted op with its permit.
#[derive(Debug)]
pub struct Admission {
    pub op: Op,
    pub permit: Permit,
}

/// What the gate admitted: every op of the batch, in order.
#[derive(Debug)]
pub struct Report {
    pub admitted: Vec<Admission>,
}

type PolicyFn = Box<dyn Fn(&Principal, RiskTier) -> Policy + Send + Sync>;

/// The one admission step. A batch is decided as a whole: one refusal refuses every op in it and
/// charges nothing.
pub struct Gate {
    policy: PolicyFn,
    quota: Option<u64>,
    used: HashMap<String, u64>,
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gate")
            .field("quota", &self.quota)
            .field("used", &self.used)
            .finish_non_exhaustive()
    }
}

impl Default for Gate {
    fn default() -> Self {
        Self::new()
    }
}

impl Gate {
    /// The default policy per tier and no quota.
    pub fn new() -> Self {
        Self::with_policy(|_, tier| default_policy(tier))
    }

    /// A gate that lets only reads pass: what to use when the real policy cannot be read.
    pub fn reads_only() -> Self {
        Self::with_policy(|_, tier| fallback_policy(tier))
    }

    /// A gate with its own policy, for example a per-agent override.
    pub fn with_policy(
        policy: impl Fn(&Principal, RiskTier) -> Policy + Send + Sync + 'static,
    ) -> Self {
        Gate {
            policy: Box::new(policy),
            quota: None,
            used: HashMap::new(),
        }
    }

    /// The default policy and at most `limit` admitted ops per workspace.
    pub fn with_quota(limit: u64) -> Self {
        Gate {
            quota: Some(limit),
            ..Self::new()
        }
    }

    /// Limit the ops admitted per workspace, keeping the policy of this gate.
    pub fn quota(mut self, limit: u64) -> Self {
        self.quota = Some(limit);
        self
    }

    /// Ops admitted so far for a workspace.
    pub fn used(&self, workspace: &str) -> u64 {
        self.used.get(workspace).copied().unwrap_or(0)
    }

    /// Decide a batch. Order of checks: shape (`invalid`), policy (`forbidden`), quota
    /// (`quota_exceeded`). Everything the gate can know without the file; the room decides the rest.
    pub fn submit(&mut self, who: &Principal, ops: Vec<Op>) -> Result<Report, Refusal> {
        check_shape(&ops)?;
        for op in &ops {
            let tier = op.tier();
            match (self.policy)(who, tier) {
                Policy::Auto => {}
                policy @ (Policy::Ask | Policy::Deny) => {
                    return Err(Refusal::Forbidden { tier, policy });
                }
            }
        }
        let used = self.used(&who.workspace);
        if let Some(limit) = self.quota
            && used + ops.len() as u64 > limit
        {
            return Err(Refusal::QuotaExceeded { limit });
        }
        self.used
            .insert(who.workspace.clone(), used + ops.len() as u64);
        let admitted = ops
            .into_iter()
            .map(|op| Admission {
                permit: Permit::new(&who.actor, &who.workspace, &op.key),
                op,
            })
            .collect();
        Ok(Report { admitted })
    }
}

fn invalid(reason: impl Into<String>) -> Refusal {
    Refusal::Invalid {
        reason: reason.into(),
    }
}

fn check_shape(ops: &[Op]) -> Result<(), Refusal> {
    if ops.is_empty() {
        return Err(invalid("the batch is empty"));
    }
    let mut keys = HashSet::new();
    for op in ops {
        if op.key.is_empty() {
            return Err(invalid("an idempotency key is empty"));
        }
        if !keys.insert(op.key.as_str()) {
            return Err(invalid(format!(
                "key {} is used twice in the batch",
                op.key
            )));
        }
        if op.deps.len() > MAX_OP_DEPS {
            return Err(invalid(format!(
                "an op names {} dependencies; the most is {MAX_OP_DEPS}",
                op.deps.len()
            )));
        }
        if op.deps.contains(&op.key) {
            return Err(invalid(format!("op {} depends on itself", op.key)));
        }
        let mut seen: HashSet<&str> = HashSet::with_capacity(op.deps.len());
        for dep in &op.deps {
            if dep.is_empty() || !seen.insert(dep.as_str()) {
                return Err(invalid(format!(
                    "op {} names an empty or repeated dependency key",
                    op.key
                )));
            }
        }
    }
    Ok(())
}
