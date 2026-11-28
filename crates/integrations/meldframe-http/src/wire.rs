//! The wire shape of an op: what a client sends over REST and over a WebSocket frame.

use meldframe_host::Op;
use meldframe_protocol::Command;
use serde::Deserialize;

/// An op as a client sends it: the idempotency key, the command, the optional precondition and dependencies.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WireOp {
    key: String,
    command: Command,
    #[serde(default)]
    if_seq: Option<u64>,
    #[serde(default)]
    deps: Vec<String>,
}

impl From<WireOp> for Op {
    fn from(w: WireOp) -> Op {
        Op {
            key: w.key,
            command: w.command,
            if_seq: w.if_seq,
            deps: w.deps,
        }
    }
}
