//! The approval seam for a call with policy Ask (ADR-0010, E2.4).
//!
//! The gate refuses an Ask call as `forbidden`. A caller that wants an approval path submits through
//! [`Approvals::submit`] instead of `Gate::submit_with`: when the gate refuses the op because the policy is
//! Ask, a pending approval is opened and the caller gets `approval_required` with its id. The owner decides
//! ([`Approvals::decide`]); the same op sent again (same workspace, actor, file and whole op: key, command, `if_seq` and dependencies) is then admitted once.
//! The match does not include the facts the tier depends on (the size of the subtree), and `get` and `find`
//! can show a Pending or Approved approval as it was until the next `submit`, `decide` or `request` marks it
//! Expired.
//!
//! Nothing here authorises the decision: `decide` takes `by` as a label and records it. Who may approve is the
//! owner's call and arrives with E2.4 and Gatehouse. The clock is the caller's (`at`, the same unit as the
//! registry's); this crate reads no wall clock. Approvals live in memory: a restart forgets them, and a
//! pending one is then asked again, which is safe. They are not pruned (a follow-up: drop the finished ones
//! older than the ttl). An approval is consumed when the gate admits the op, not when the room applies it, so an
//! op the room then refuses has spent it; a consumed approval does not block a new one for the same op.
//!
//! Decisions have their own log ([`Approvals::decisions`]), not the activity log: an activity entry carries
//! the sequence number of an accepted op, the log requires those to rise, and its gaps mean lost writes. A
//! decision is not an op and has no sequence number.

use meldframe_host::{Gate, NodeFacts, Op, Principal, Refusal, Report};
use meldframe_module::{Policy, RiskTier};
use meldframe_rooms::{ActivityEntry, ActivityKind};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Where an approval is. `Consumed` is the end of an approved one: it let one admission through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalState {
    Pending,
    Approved,
    Denied,
    Expired,
    Consumed,
}

/// The owner's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Approve,
    Deny,
}

/// One approval: the op it covers (workspace, actor, file, and a fingerprint of the whole op: key, command, `if_seq` and dependencies) and where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Approval {
    pub id: String,
    pub workspace: String,
    pub actor: String,
    pub file: String,
    pub key: String,
    /// The command summary: its kind and the node it is about.
    pub kind: ActivityKind,
    pub node_id: String,
    pub state: ApprovalState,
    pub created_at: u64,
    /// The approval is dead from this clock value on, whether it is pending or approved.
    pub expires_at: u64,
    command: String,
}

/// One entry of the decision log: an approval turned Approved, Denied or Expired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub approval: String,
    pub actor: String,
    pub file: String,
    pub key: String,
    pub kind: ActivityKind,
    pub node_id: String,
    /// Approved, Denied or Expired.
    pub state: ApprovalState,
    /// Who decided; `None` for an expiry.
    pub by: Option<String>,
    pub at: u64,
}

/// Why a call through [`Approvals::submit`] was refused. Codes are stable (`docs/error-codes.md`).
#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalRefusal {
    /// The policy is Ask and the owner has not approved: wait for the decision, then send the op again.
    Required { id: String },
    /// The owner denied this op. Send a new op (a new key) to ask again.
    Denied { id: String },
    /// The approval ran out before it was used. Send a new op (a new key) to ask again.
    Expired { id: String },
    /// The gate refused for another reason (Deny policy, quota, shape); it keeps its own code.
    Refused(Refusal),
}

impl ApprovalRefusal {
    pub fn code(&self) -> &'static str {
        match self {
            ApprovalRefusal::Required { .. } => "approval_required",
            ApprovalRefusal::Denied { .. } => "approval_denied",
            ApprovalRefusal::Expired { .. } => "approval_expired",
            ApprovalRefusal::Refused(r) => r.code(),
        }
    }
}

impl fmt::Display for ApprovalRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApprovalRefusal::Required { id } => {
                write!(f, "approval required: approval {id} waits for the owner")
            }
            ApprovalRefusal::Denied { id } => write!(f, "approval denied: approval {id}"),
            ApprovalRefusal::Expired { id } => write!(f, "approval expired: approval {id}"),
            ApprovalRefusal::Refused(r) => write!(f, "{r}"),
        }
    }
}

impl std::error::Error for ApprovalRefusal {}

/// Why `decide` did nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecideError {
    Unknown {
        id: String,
    },
    /// A decision is final: the approval is not pending.
    NotPending {
        id: String,
        state: ApprovalState,
    },
    /// The approval ran out before the decision.
    Expired {
        id: String,
    },
}

impl fmt::Display for DecideError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecideError::Unknown { id } => write!(f, "approval {id} is unknown"),
            DecideError::NotPending { id, state } => {
                write!(f, "approval {id} is already {state:?}")
            }
            DecideError::Expired { id } => write!(f, "approval {id} expired"),
        }
    }
}

impl std::error::Error for DecideError {}

/// A gate that [`Approvals::submit`] can admit an approved op through. It is a `Gate` (it derefs to one, so
/// `submit`, `submit_with` and `used` work as usual) with a flag of its own that only `submit` sets.
#[derive(Debug)]
pub struct ApprovalGate {
    gate: Gate,
    once: Arc<AtomicBool>,
}

impl ApprovalGate {
    /// At most `limit` admitted ops per workspace, as [`Gate::quota`].
    pub fn quota(mut self, limit: u64) -> Self {
        self.gate = self.gate.quota(limit);
        self
    }
}

impl std::ops::Deref for ApprovalGate {
    type Target = Gate;
    fn deref(&self) -> &Gate {
        &self.gate
    }
}

impl std::ops::DerefMut for ApprovalGate {
    fn deref_mut(&mut self) -> &mut Gate {
        &mut self.gate
    }
}

/// Holds a gate's flag up and drops it again, also when a policy or a facts call panics in between.
struct Lift<'a>(&'a AtomicBool);

impl<'a> Lift<'a> {
    fn new(flag: &'a AtomicBool) -> Self {
        flag.store(true, Ordering::SeqCst);
        Lift(flag)
    }
}

impl Drop for Lift<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// The approvals of a process, and the log of their decisions.
#[derive(Debug)]
pub struct Approvals {
    ttl: u64,
    items: Vec<Approval>,
    log: Vec<Decision>,
}

impl Approvals {
    /// Approvals that live `ttl` clock units from the request.
    pub fn new(ttl: u64) -> Self {
        Approvals {
            ttl,
            items: Vec::new(),
            log: Vec::new(),
        }
    }

    /// A gate with the policy `base` (chain `.quota(n)` for a quota) that can admit an approved op: while
    /// [`Approvals::submit`] admits one through it, and only then, an Ask from `base` reads as Auto. The flag
    /// belongs to this one gate, and `submit` holds `&mut` on it, so no other gate and no other call sees it
    /// lifted. Deny and the quota stay as `base` and the gate say, and the approved op is counted in the gate
    /// like any other. `submit` takes only this kind of gate.
    pub fn gate(
        &self,
        base: impl Fn(&Principal, RiskTier) -> Policy + Send + Sync + 'static,
    ) -> ApprovalGate {
        let once = Arc::new(AtomicBool::new(false));
        let lifted = Arc::clone(&once);
        let gate = Gate::with_policy(move |who, tier| match base(who, tier) {
            Policy::Ask if lifted.load(Ordering::SeqCst) => Policy::Auto,
            policy => policy,
        });
        ApprovalGate { gate, once }
    }

    /// Admit `op` as `gate` does, and when the only obstacle is the Ask policy, work through the approval.
    /// One op per call (an approval covers one op). A gate that admits the op needs no approval; any other
    /// refusal (Deny, quota, shape) stays as it is, and an approval never lifts it: the approved op is
    /// admitted only after the gate itself has said Ask. An approved op goes through the same gate again with
    /// its Ask read as Auto (see [`Approvals::gate`]), so the facts, the tier and the quota are the real
    /// ones, and the approval is consumed there, at admission: an op the room then refuses has spent it.
    pub fn submit(
        &mut self,
        gate: &mut ApprovalGate,
        who: &Principal,
        file: &str,
        op: Op,
        facts: &dyn NodeFacts,
        at: u64,
    ) -> Result<Report, ApprovalRefusal> {
        self.expire_due(at);
        match gate.submit_with(who, vec![op.clone()], facts) {
            Ok(report) => {
                // Admitted without the approval (the policy or the tier changed): an approval for this
                // op is spent with it, so it cannot admit the op again later.
                if let Some(i) = self.matching(who, file, &op, Some(ApprovalState::Approved)) {
                    self.items[i].state = ApprovalState::Consumed;
                }
                return Ok(report);
            }
            Err(Refusal::Forbidden {
                policy: Policy::Ask,
                ..
            }) => {}
            Err(other) => return Err(ApprovalRefusal::Refused(other)),
        }
        let i = self.slot(who, file, &op, at);
        let id = self.items[i].id.clone();
        match self.items[i].state {
            // `slot` never returns a consumed approval; were it to, asking again is the safe answer.
            ApprovalState::Pending | ApprovalState::Consumed => {
                Err(ApprovalRefusal::Required { id })
            }
            ApprovalState::Denied => Err(ApprovalRefusal::Denied { id }),
            ApprovalState::Expired => Err(ApprovalRefusal::Expired { id }),
            ApprovalState::Approved => {
                let admitted = {
                    let flag = Arc::clone(&gate.once);
                    let _lift = Lift::new(&flag);
                    gate.submit_with(who, vec![op], facts)
                };
                let report = admitted.map_err(ApprovalRefusal::Refused)?;
                self.items[i].state = ApprovalState::Consumed;
                Ok(report)
            }
        }
    }

    /// Open a pending approval for `op`, or return the one that already covers it (same workspace, actor, file
    /// and whole op, not yet consumed): asking again never opens a second one.
    pub fn request(&mut self, who: &Principal, file: &str, op: &Op, at: u64) -> String {
        self.expire_due(at);
        let i = self.slot(who, file, op, at);
        self.items[i].id.clone()
    }

    /// The owner's answer. Only a pending approval takes one, once. `by` is recorded, not checked.
    pub fn decide(
        &mut self,
        id: &str,
        verdict: Verdict,
        by: &str,
        at: u64,
    ) -> Result<&Approval, DecideError> {
        self.expire_due(at);
        let unknown = || DecideError::Unknown { id: id.into() };
        let i = self
            .items
            .iter()
            .position(|a| a.id == id)
            .ok_or_else(unknown)?;
        match self.items[i].state {
            ApprovalState::Pending => {}
            ApprovalState::Expired => return Err(DecideError::Expired { id: id.into() }),
            state => {
                return Err(DecideError::NotPending {
                    id: id.into(),
                    state,
                });
            }
        }
        let state = match verdict {
            Verdict::Approve => ApprovalState::Approved,
            Verdict::Deny => ApprovalState::Denied,
        };
        self.items[i].state = state;
        self.record(i, state, Some(by.to_string()), at);
        Ok(&self.items[i])
    }

    pub fn get(&self, id: &str) -> Option<&Approval> {
        self.items.iter().find(|a| a.id == id)
    }

    /// The latest approval of an actor under a key, in a workspace and file.
    pub fn find(&self, workspace: &str, file: &str, actor: &str, key: &str) -> Option<&Approval> {
        self.items.iter().rev().find(|a| {
            a.workspace == workspace && a.file == file && a.actor == actor && a.key == key
        })
    }

    /// Every decision so far, in order.
    pub fn decisions(&self) -> &[Decision] {
        &self.log
    }

    /// The latest approval of this op (same workspace, actor, file and whole op) that is not consumed, in
    /// `state` when one is given.
    fn matching(
        &self,
        who: &Principal,
        file: &str,
        op: &Op,
        state: Option<ApprovalState>,
    ) -> Option<usize> {
        let command = format!("{op:?}");
        self.items.iter().rposition(|a| {
            a.workspace == who.workspace
                && a.actor == who.actor
                && a.file == file
                && a.key == op.key
                && a.command == command
                && a.state != ApprovalState::Consumed
                && state.is_none_or(|s| a.state == s)
        })
    }

    /// The approval covering this op, opened when there is none.
    fn slot(&mut self, who: &Principal, file: &str, op: &Op, at: u64) -> usize {
        if let Some(i) = self.matching(who, file, op, None) {
            return i;
        }
        let command = format!("{op:?}");
        let (kind, node_id) = ActivityEntry::describe(&op.command);
        self.items.push(Approval {
            id: format!("apr_{}", self.items.len() + 1),
            workspace: who.workspace.clone(),
            actor: who.actor.clone(),
            file: file.into(),
            key: op.key.clone(),
            kind,
            node_id,
            state: ApprovalState::Pending,
            created_at: at,
            expires_at: at.saturating_add(self.ttl),
            command,
        });
        self.items.len() - 1
    }

    /// Mark every pending or approved approval that is past its expiry.
    fn expire_due(&mut self, at: u64) {
        for i in 0..self.items.len() {
            let a = &mut self.items[i];
            if matches!(a.state, ApprovalState::Pending | ApprovalState::Approved)
                && at >= a.expires_at
            {
                a.state = ApprovalState::Expired;
                self.record(i, ApprovalState::Expired, None, at);
            }
        }
    }

    fn record(&mut self, i: usize, state: ApprovalState, by: Option<String>, at: u64) {
        let a = &self.items[i];
        self.log.push(Decision {
            approval: a.id.clone(),
            actor: a.actor.clone(),
            file: a.file.clone(),
            key: a.key.clone(),
            kind: a.kind,
            node_id: a.node_id.clone(),
            state,
            by,
            at,
        });
    }
}
