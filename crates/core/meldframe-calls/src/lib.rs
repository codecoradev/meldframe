//! The call log: one entry for every call to a tool, written or refused, with the decision the policy made and how
//! the call ended. It is the answer to "what did this actor try, and what did the gate say". An entry holds names
//! and codes only (no node names, texts or commands): a person can read the log without reading the design.
//!
//! The activity log of `meldframe-rooms` is a different record: one entry per op that was written, kept for the
//! history of a file. This one is per call and includes the calls that wrote nothing.

use meldframe_host::{Principal, Refusal};
use meldframe_module::{Policy, RiskTier};
use std::collections::VecDeque;

/// What the policy did with a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// The call went on to the room (it can still fail there, with its own outcome).
    Allow,
    /// The policy asks the owner first; without an approval path the call is refused.
    Ask,
    /// The policy refuses.
    Deny,
}

impl Decision {
    /// The stable lower-case name.
    pub fn as_str(self) -> &'static str {
        match self {
            Decision::Allow => "allow",
            Decision::Ask => "ask",
            Decision::Deny => "deny",
        }
    }
}

/// One call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallEntry {
    /// The clock value the server gave it (milliseconds since the epoch in the server).
    pub at: u64,
    pub workspace: String,
    pub actor: String,
    pub tool: String,
    pub file: String,
    /// The tier of the call: the tool's, or the tier that was refused. `None` when no tier could be told.
    pub risk: Option<RiskTier>,
    /// What the policy decided. `None` when the call failed before a policy was asked (a malformed call).
    pub decision: Option<Decision>,
    /// `ok`, or the stable code of the refusal or failure.
    pub outcome: String,
}

/// The seam: keep calls, give back the newest.
pub trait CallLog: Send + std::fmt::Debug {
    fn record(&mut self, entry: CallEntry);

    /// At most `limit` of the newest entries, oldest first.
    fn recent(&self, limit: usize) -> Vec<CallEntry>;
}

/// A log in memory that keeps the newest `capacity` entries.
#[derive(Debug)]
pub struct MemoryCallLog {
    capacity: usize,
    entries: VecDeque<CallEntry>,
}

impl MemoryCallLog {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: VecDeque::new(),
        }
    }
}

impl CallLog for MemoryCallLog {
    fn record(&mut self, entry: CallEntry) {
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    fn recent(&self, limit: usize) -> Vec<CallEntry> {
        let skip = self.entries.len().saturating_sub(limit);
        self.entries.iter().skip(skip).cloned().collect()
    }
}

/// How a call reads in the log: the tier, what the policy decided, and the outcome code. `done` is `Ok(())` for a call
/// that went through, `Err(Some(refusal))` for one a policy or the room refused, `Err(None)` for a failure that is
/// not a refusal (the store is down). A refusal by the policy (`Forbidden`, a quota) is `Ask` or `Deny`; a refusal by
/// the room after the gate let the call through (a conflict, a command the core refused) is `Allow` with that code;
/// a malformed call or a failure that is not a refusal has no decision.
pub fn describe(
    done: Result<(), Option<&Refusal>>,
    tool_tier: Option<RiskTier>,
) -> (Option<RiskTier>, Option<Decision>, String) {
    match done {
        Ok(()) => (tool_tier, Some(Decision::Allow), "ok".into()),
        Err(Some(Refusal::Forbidden { tier, policy })) => {
            let decision = if *policy == Policy::Ask {
                Decision::Ask
            } else {
                Decision::Deny
            };
            (Some(*tier), Some(decision), "forbidden".into())
        }
        Err(Some(
            r @ (Refusal::QuotaExceeded { .. }
            | Refusal::RateExceeded { .. }
            | Refusal::StorageExceeded { .. }),
        )) => (tool_tier, Some(Decision::Deny), r.code().into()),
        // A malformed call: no policy decision was made on it.
        Err(Some(r @ (Refusal::Invalid { .. } | Refusal::BatchTooLarge { .. }))) => {
            (tool_tier, None, r.code().into())
        }
        Err(Some(r)) => (tool_tier, Some(Decision::Allow), r.code().into()),
        Err(None) => (tool_tier, None, "unavailable".into()),
    }
}

/// How a call that has only a stable code reads in the log (a tool that does not go through the gate, such as
/// `whoami` or `capture`): `None` is `ok` and allowed; `forbidden` is denied; `invalid` and `bad_request` are malformed
/// calls with no decision; any other code is a call the policy let through that then failed with it.
/// A code that is not 1 to 64 of `a-z`, `0-9` and `_` (a tool that put text there) is logged as `internal`.
pub fn describe_code(code: Option<&str>) -> (Option<Decision>, String) {
    let stable = |c: &str| {
        (1..=64).contains(&c.len())
            && c.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    };
    let code = code.map(|c| if stable(c) { c } else { "internal" });
    match code {
        None => (Some(Decision::Allow), "ok".into()),
        Some("forbidden") => (Some(Decision::Deny), "forbidden".into()),
        Some(c @ ("invalid" | "bad_request")) => (None, c.into()),
        Some(c) => (Some(Decision::Allow), c.into()),
    }
}

impl CallEntry {
    /// The entry for a call. Names are cut at 128 characters, so a caller cannot grow an entry by what it sends.
    pub fn new(
        at: u64,
        who: &Principal,
        tool: &str,
        file: &str,
        tool_tier: Option<RiskTier>,
        done: Result<(), Option<&Refusal>>,
    ) -> Self {
        let cap = |s: &str| s.chars().take(128).collect::<String>();
        let (risk, decision, outcome) = describe(done, tool_tier);
        Self {
            at,
            workspace: cap(&who.workspace),
            actor: cap(&who.actor),
            tool: cap(tool),
            file: cap(file),
            risk,
            decision,
            outcome,
        }
    }

    /// The entry for a call that has only a stable code (see [`describe_code`]).
    pub fn with_code(
        at: u64,
        who: &Principal,
        tool: &str,
        file: &str,
        tier: Option<RiskTier>,
        code: Option<&str>,
    ) -> Self {
        let cap = |s: &str| s.chars().take(128).collect::<String>();
        let (decision, outcome) = describe_code(code);
        Self {
            at,
            workspace: cap(&who.workspace),
            actor: cap(&who.actor),
            tool: cap(tool),
            file: cap(file),
            risk: tier,
            decision,
            outcome,
        }
    }
}
