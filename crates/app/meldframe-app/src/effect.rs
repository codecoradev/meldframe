//! Work only the platform can do. The shell carries it out and reports the result as an action.

use meldframe_protocol::Command;
use serde::{Deserialize, Serialize};

/// An op for the server. Sending it again with the same `key` is safe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutOp {
    /// Idempotency key.
    pub key: String,
    pub command: Command,
    /// Keys of the person's earlier ops this one is built on; the server accepts it only if all were applied.
    pub deps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[allow(
    clippy::large_enum_variant,
    reason = "short-lived values; boxing would leak into every consumer pattern"
)]
pub enum Effect {
    /// Send these ops to the server, in order. The answer comes back as `Action::Frame`.
    SendOps { ops: Vec<OutOp> },
    /// Open the subscription (a new one, or again on the stream the shell holds) and replay the ops after
    /// `after_seq`. What arrives comes back as `Action::Connected` and `Action::Frame`.
    Subscribe { after_seq: u64 },
    /// Wait `after_ms`, then dispatch `Action::Reconnect`.
    Reconnect { after_ms: u64 },
    /// Wait `after_ms`, then dispatch `Action::Tick`: the app has a retry to send.
    WakeAfter { after_ms: u64 },
    /// Redraw. The nodes named changed; an empty list means everything.
    Render { nodes: Vec<String> },
}
