//! Work only the platform can do. The shell carries it out and reports the result as an action.

use crate::activity::ReviewState;
use meldframe_protocol::Command;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// An op for the server. Sending it again with the same `key` is safe.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutOp {
    /// Idempotency key.
    pub key: String,
    pub command: Command,
    /// Keys of the person's earlier ops this one is built on; the server accepts it only if all were applied.
    pub deps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
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
    Subscribe {
        #[ts(type = "number")]
        after_seq: u64,
    },
    /// Wait `after_ms`, then dispatch `Action::Reconnect`.
    Reconnect {
        #[ts(type = "number")]
        after_ms: u64,
    },
    /// Wait `after_ms`, then dispatch `Action::Tick`: the app has a retry to send.
    WakeAfter {
        #[ts(type = "number")]
        after_ms: u64,
    },
    /// `GET /v1/files/{file}/batches?before={before_n}&after={after_n}&limit={limit}` (a missing bound is left out).
    /// The answer comes back as `Action::BatchesLoaded`, a failure as `Action::BatchRequestFailed`.
    FetchBatches {
        before_n: Option<u32>,
        after_n: Option<u32>,
        limit: u32,
    },
    /// `PUT /v1/files/{file}/batches/{n}/review` with `{"state": state}`. The row it answers comes back as
    /// `Action::BatchReviewed`, a refusal as
    /// `Action::BatchRequestFailed`.
    PutBatchReview { n: u32, state: ReviewState },
    /// Redraw. The nodes named changed; an empty list means everything.
    Render { nodes: Vec<String> },
}

impl From<meldframe_activity::Fetch> for Effect {
    fn from(f: meldframe_activity::Fetch) -> Self {
        Effect::FetchBatches {
            before_n: f.before_n,
            after_n: f.after_n,
            limit: f.limit,
        }
    }
}
