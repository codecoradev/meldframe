//! One op as a transport submits it.

use crate::NodeFacts;
use meldframe_module::RiskTier;
use meldframe_protocol::Command;

/// A command with the idempotency key, the optimistic precondition and the dependencies that the wire
/// carries (`ClientMsg::Op`). The actor is not part of an op: it comes from the principal.
#[derive(Debug, Clone)]
pub struct Op {
    /// Idempotency key, unique per actor. Sending the op again with the same key is safe.
    pub key: String,
    pub command: Command,
    /// Refuse with `conflict` when a node the command touches changed after this sequence number.
    pub if_seq: Option<u64>,
    /// Keys of the same actor's ops that must have been applied.
    pub deps: Vec<String>,
}

impl Op {
    pub fn new(key: impl Into<String>, command: Command) -> Self {
        Op {
            key: key.into(),
            command,
            if_seq: None,
            deps: Vec::new(),
        }
    }

    pub fn with_if_seq(mut self, seq: u64) -> Self {
        self.if_seq = Some(seq);
        self
    }

    pub fn with_deps<I, S>(mut self, deps: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.deps = deps.into_iter().map(Into::into).collect();
        self
    }

    /// The worst tier of this op, without the file: removing a node is Delete, every other command is Change.
    pub fn tier(&self) -> RiskTier {
        self.tier_with(&())
    }

    /// The tier of this op given facts about the tree before it: ADR-0010 relaxes a delete that `facts`
    /// calls small to Change. Facts never raise a tier, and an unknown node is a Delete (fail closed).
    pub fn tier_with(&self, facts: &dyn NodeFacts) -> RiskTier {
        match &self.command {
            Command::RemoveNode { id } if !facts.delete_is_small(id) => RiskTier::Delete,
            _ => RiskTier::Change,
        }
    }

    /// The tier of each op of a batch: `tier_with`, except that a batch that adds or reparents a node gets no
    /// relief (no facts): it could grow the subtree it then removes.
    pub fn batch_tiers(ops: &[Op], facts: &dyn NodeFacts) -> Vec<RiskTier> {
        let grows = ops.iter().any(|o| {
            matches!(
                o.command,
                Command::AddNode { .. } | Command::Reparent { .. }
            )
        });
        let facts: &dyn NodeFacts = if grows { &() } else { facts };
        ops.iter().map(|op| op.tier_with(facts)).collect()
    }
}
