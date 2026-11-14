//! One op as a transport submits it.

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

    /// How much this op can change: removing a node is Delete, every other command is Change. ADR-0010 relaxes
    /// a small delete to Auto; that needs the file, which the gate does not have, so every delete is Delete here.
    pub fn tier(&self) -> RiskTier {
        match self.command {
            Command::RemoveNode { .. } => RiskTier::Delete,
            Command::UpdateNode { .. }
            | Command::AddNode { .. }
            | Command::Reparent { .. }
            | Command::Reorder { .. } => RiskTier::Change,
        }
    }
}
