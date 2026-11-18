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

/// One admitted op sealed with its permit. Only the gate builds one and the fields are private, so an
/// op cannot be paired with another op's permit, and the room takes the whole value.
#[derive(Debug)]
pub struct Admission {
    op: Op,
    permit: Permit,
}

impl Admission {
    /// The admitted op.
    pub fn op(&self) -> &Op {
        &self.op
    }

    /// The permit the gate gave the op.
    pub fn permit(&self) -> &Permit {
        &self.permit
    }

    pub(crate) fn into_parts(self) -> (Op, Permit) {
        (self.op, self.permit)
    }
}

/// What the gate admitted: every op of the batch, in order.
#[derive(Debug)]
pub struct Report {
    pub admitted: Vec<Admission>,
}

/// What the gate may know about the file's tree, read at submit time, so that the tier of an op can depend on
/// it (ADR-0010) without the gate holding the file state. The room registry gives the implementation.
pub trait NodeFacts {
    /// True when removing `node_id` is a small delete: the node is in the file, is not a frame, and its
    /// subtree is within the limit. False for anything else, including a node the facts do not know.
    fn delete_is_small(&self, node_id: &str) -> bool;
}

/// No facts: no delete is small.
impl NodeFacts for () {
    fn delete_is_small(&self, _: &str) -> bool {
        false
    }
}

const MINUTE_MS: u64 = 60_000;

type UsageFn = Box<dyn Fn(&str) -> Option<(u64, u64)> + Send + Sync>;

type Clock = Box<dyn Fn() -> u64 + Send + Sync>;

type PolicyFn = Box<dyn Fn(&Principal, RiskTier) -> Policy + Send + Sync>;

/// The one admission step. A batch is decided as a whole: one refusal refuses every op in it and
/// charges nothing.
pub struct Gate {
    policy: PolicyFn,
    quota: Option<u64>,
    used: HashMap<String, u64>,
    /// Ops per minute per workspace, with the clock (milliseconds) that tells the minute.
    rate: Option<(u64, Clock)>,
    /// Per workspace: the minute window and the ops admitted in it.
    windows: HashMap<String, (u64, u64)>,
    batch_limit: Option<usize>,
    storage_limit: Option<u64>,
    stored: HashMap<String, u64>,
    /// What a workspace wrote before this gate existed, asked once per workspace (see [`Gate::usage`]).
    usage: Option<UsageFn>,
    seen: HashSet<String>,
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
            rate: None,
            windows: HashMap::new(),
            batch_limit: None,
            storage_limit: None,
            stored: HashMap::new(),
            usage: None,
            seen: HashSet::new(),
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

    /// Let each workspace send at most `limit` ops in a minute. `now_ms` is the clock: milliseconds from any fixed
    /// origin; a minute window is `now_ms / 60000`, and the count starts again at the next one.
    pub fn ops_per_minute(
        mut self,
        limit: u64,
        now_ms: impl Fn() -> u64 + Send + Sync + 'static,
    ) -> Self {
        self.rate = Some((limit, Box::new(now_ms)));
        self
    }

    /// Refuse a batch of more than `limit` ops as `batch_too_large`.
    pub fn batch_limit(mut self, limit: usize) -> Self {
        self.batch_limit = Some(limit);
        self
    }

    /// Let each workspace write at most `limit` bytes of ops (the JSON size of each command).
    pub fn storage_limit(mut self, limit: u64) -> Self {
        self.storage_limit = Some(limit);
        self
    }

    /// Start each workspace from what it already wrote: `usage(workspace)` returns its `(ops, bytes)` and is asked
    /// once, the first time a well-formed call names the workspace. A restarted server passes what the store
    /// holds, so the total and the storage quotas survive it; `None` (the store could not say) is asked again on the next call. The per-minute count is not seeded: it starts empty.
    pub fn usage(
        mut self,
        usage: impl Fn(&str) -> Option<(u64, u64)> + Send + Sync + 'static,
    ) -> Self {
        self.usage = Some(Box::new(usage));
        self
    }

    /// Bytes of ops admitted so far for a workspace.
    pub fn storage_used(&self, workspace: &str) -> u64 {
        self.stored.get(workspace).copied().unwrap_or(0)
    }

    /// Ops admitted so far for a workspace.
    pub fn used(&self, workspace: &str) -> u64 {
        self.used.get(workspace).copied().unwrap_or(0)
    }

    /// Decide a batch. Order of checks: shape (`invalid`), batch limit (`batch_too_large`), policy (`forbidden`),
    /// total quota (`quota_exceeded`), rate (`quota_ops_per_minute`), storage (`quota_storage`). Everything the gate
    /// can know without the file; the room decides the rest. Nothing is charged unless every check passes.
    pub fn submit(&mut self, who: &Principal, ops: Vec<Op>) -> Result<Report, Refusal> {
        self.submit_with(who, ops, &())
    }

    /// Like [`Gate::submit`], with facts about the tree before the batch, so that a small delete is Change
    /// (see [`Op::batch_tiers`]). The caller must pass the facts of the room the ops target.
    pub fn submit_with(
        &mut self,
        who: &Principal,
        ops: Vec<Op>,
        facts: &dyn NodeFacts,
    ) -> Result<Report, Refusal> {
        check_shape(&ops)?;
        if let Some(limit) = self.batch_limit
            && ops.len() > limit
        {
            return Err(Refusal::BatchTooLarge {
                limit: limit as u64,
            });
        }
        let tiers = Op::batch_tiers(&ops, facts);
        for tier in tiers.iter().copied() {
            match (self.policy)(who, tier) {
                Policy::Auto => {}
                policy @ (Policy::Ask | Policy::Deny) => {
                    return Err(Refusal::Forbidden { tier, policy });
                }
            }
        }
        if let Some(usage) = &self.usage
            && !self.seen.contains(&who.workspace)
            && let Some((ops, bytes)) = usage(&who.workspace)
        {
            self.seen.insert(who.workspace.clone());
            self.used.insert(who.workspace.clone(), ops);
            self.stored.insert(who.workspace.clone(), bytes);
        }
        let used = self.used(&who.workspace);
        if let Some(limit) = self.quota
            && used + ops.len() as u64 > limit
        {
            return Err(Refusal::QuotaExceeded { limit });
        }
        let window = match &self.rate {
            Some((limit, now)) => {
                let t = now();
                // A clock that steps back keeps the newer window it already counted in: no reset, no bypass.
                let (w, count) = match self.windows.get(&who.workspace) {
                    Some(&(at, n)) if at >= t / MINUTE_MS => (at, n),
                    _ => (t / MINUTE_MS, 0),
                };
                if count.saturating_add(ops.len() as u64) > *limit {
                    return Err(Refusal::RateExceeded {
                        limit: *limit,
                        retry_after_ms: (w + 1).saturating_mul(MINUTE_MS).saturating_sub(t),
                    });
                }
                Some((w, count))
            }
            None => None,
        };
        let bytes: u64 = ops
            .iter()
            .map(|o| serde_json::to_vec(&o.command).map_or(0, |v| v.len() as u64))
            .sum();
        let stored = self.storage_used(&who.workspace);
        if let Some(limit) = self.storage_limit
            && stored.saturating_add(bytes) > limit
        {
            return Err(Refusal::StorageExceeded { limit });
        }
        // Everything passed: charge.
        if let Some((w, count)) = window {
            self.windows
                .insert(who.workspace.clone(), (w, count + ops.len() as u64));
        }
        self.stored.insert(who.workspace.clone(), stored + bytes);
        self.used
            .insert(who.workspace.clone(), used + ops.len() as u64);
        let admitted = ops
            .into_iter()
            .zip(tiers)
            .map(|(op, tier)| Admission {
                permit: Permit::new(&who.actor, &who.workspace, &op.key, tier),
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
