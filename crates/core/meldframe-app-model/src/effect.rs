//! Work only the platform can do. The shell carries it out and reports the result as an action.

use crate::activity::ReviewState;
use crate::checkpoint::CheckpointKind;
use crate::raster::{RasterRect, RasterSink};
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
    /// `GET /v1/assets?limit={limit}&after={after}` (`after` is left out for the first page): the assets of the workspace.
    /// The answer comes back as `Action::AssetsLoaded` (with the same `after` and `request`), a failure as
    /// `Action::AssetsRequestFailed` (with the same `request`). `request` is the number of this request: only the answer
    /// that carries the latest one is taken.
    FetchAssets {
        after: Option<String>,
        limit: u32,
        request: u32,
    },
    /// `GET /v1/files/{file}/checkpoints`. The answer comes back as `Action::CheckpointsLoaded`, a failure as
    /// `Action::CheckpointRequestFailed`.
    FetchCheckpoints {},
    /// `POST /v1/files/{file}/checkpoints` with `{"key", "kind", "seq", "label"?, "about"?}`; sending it again with the
    /// same `key` is safe. The row comes back as `Action::CheckpointPosted` (with this `key`), a refusal as
    /// `Action::CheckpointRequestFailed`.
    PostCheckpoint {
        key: String,
        kind: CheckpointKind,
        label: Option<String>,
        about: Option<String>,
        #[ts(type = "number")]
        seq: u64,
    },
    /// `GET /v1/files/{file}/checkpoints/{n}/state`. The file at the checkpoint comes back as
    /// `Action::CheckpointStateLoaded`, a failure as `Action::CheckpointRequestFailed`.
    FetchCheckpointState { n: u32 },
    /// Draw `rect` (world space) at `scale` as a PNG of `width` by `height` pixels, offscreen and without the overlay, and
    /// hold the bytes for `to`. The shell reports the size it drew as `Wire::Drawn` (or a failure) with this `request`; the
    /// bytes go nowhere until `Effect::FinishRaster`. `file` is the name to save a download under, safe to use as it is.
    Raster {
        request: u32,
        rect: RasterRect,
        scale: f64,
        width: u32,
        height: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        file: Option<String>,
        to: RasterSink,
    },
    /// What the app decided about the image it holds for `request`: `deliver` it to its sink (the answer comes back as
    /// `Wire::Delivered`) or drop it (it was refused after drawing).
    FinishRaster { request: u32, deliver: bool },
    /// Answer the capture request `request_id` with `ClientMsg::CaptureFailed` (`code` is `node_not_found`, `too_large`,
    /// `stale_client`, `invalid_scale`, ...: the server maps one it does not know to `render_failed`).
    FailCapture {
        request_id: String,
        code: String,
        message: String,
    },
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
