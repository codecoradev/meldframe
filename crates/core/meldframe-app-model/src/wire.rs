//! What the shell hands the app layer as the result of an effect or as a frame it did not read (audit finding T6,
//! issue 323): the status and the text of the answer, as the network gave them. The app layer parses and checks them
//! (`meldframe-app-parts`, `answers`: serde with the bounds of the server), so a front end passes raw text through and
//! checks nothing of its own. An answer that is not what the server sends (a proxy's page, a body of the wrong shape or
//! out of bound) is the failure of its request, with the code `unavailable`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The largest whole number a JavaScript front end holds exactly (2^53 - 1): no size, counter or time of the server is
/// larger, so an answer that holds a bigger one is not the server's.
pub const MAX_SAFE_INT: u64 = (1 << 53) - 1;

/// A raw answer or frame. `body` and `text` are the text of what was received, not parsed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Wire {
    /// The answer of `Effect::FetchBatches`: `BatchesLoaded` for a page, `BatchRequestFailed` for anything else.
    Batches { status: u16, body: String },
    /// The answer of `Effect::PutBatchReview`: `BatchReviewed` with the row alone, or `BatchRequestFailed`.
    BatchReview { status: u16, body: String },
    /// The answer of `Effect::FetchCheckpoints`: `CheckpointsLoaded`, or `CheckpointRequestFailed` for the list.
    Checkpoints { status: u16, body: String },
    /// The answer of `Effect::PostCheckpoint` with this `key`: `CheckpointPosted`, or `CheckpointRequestFailed` for the post.
    CheckpointPosted {
        key: String,
        status: u16,
        body: String,
    },
    /// The answer of `Effect::FetchCheckpointState` for number `n`: `CheckpointStateLoaded`, or `CheckpointRequestFailed`.
    CheckpointState { n: u32, status: u16, body: String },
    /// The answer of `Effect::FetchAssets` with its `request` and `after`: `AssetsLoaded`, or `AssetsRequestFailed`.
    Assets {
        request: u32,
        after: Option<String>,
        status: u16,
        body: String,
    },
    /// A `batch`, `touch` or `checkpoint` frame of the live socket, as the socket sent it: `Action::Frame` with the frame
    /// made of it. These are hints over the log, so a frame that does not pass the check is ignored (a fetch heals the gap);
    /// the app treats a text of any other type as nothing.
    Hint {
        #[ts(type = "number")]
        now_ms: u64,
        text: String,
    },
    /// An `error` frame of the live socket that answers the last `subscribe` the shell sent (the shell matches the id of
    /// that subscribe, which the app does not know): `Action::Frame` with `Frame::SubscribeRefused`, which the app answers
    /// with a new subscribe, a wait or `Disconnected` (audit finding T14, issue 324). A text that is not an `error` frame
    /// is nothing.
    SubscribeRefused {
        #[ts(type = "number")]
        now_ms: u64,
        text: String,
    },
    /// The answer of the REST call that sent the op with this `key`, when its status is not 200 (the live event confirms an
    /// accepted op, so the shell reports nothing for a 200; the app takes a 200 as nothing too): `Action::Frame` with
    /// `Frame::Rejected`, whose code and retry flag the app reads from the body. A body that is not the server's (a proxy's
    /// page, no `error`, a code or a flag of the wrong type) is the code `unavailable`, and a missing or mistyped flag
    /// counts as retryable, so the app sends the op again by its own rule (audit finding T14, issue 324).
    OpRefused {
        #[ts(type = "number")]
        now_ms: u64,
        key: String,
        status: u16,
        body: String,
    },
    /// A `captureRequest` frame of the live socket, as the socket sent it: `Action::Png` with `PngOp::Capture` made of it. A
    /// text that is not one, or has an id or a token that is not safe to put in a path and a header, is nothing.
    Capture {
        #[ts(type = "number")]
        now_ms: u64,
        text: String,
    },
    /// The answer of `Effect::Raster` with this `request`: `status` 200 and a body `{"width", "height", "bytes"}` (what the
    /// shell drew and holds) is `PngOp::Drawn`, anything else `PngOp::DrawFailed` with the code of `{"error": {"code"}}` (else
    /// `render_failed`).
    Drawn {
        request: u32,
        status: u16,
        body: String,
    },
    /// The answer of the sink of `Effect::FinishRaster` with this `request`: `status` 200 is `PngOp::Delivered`, anything else
    /// `PngOp::DeliveryFailed` (the upload's status and body as the network gave them).
    Delivered {
        request: u32,
        status: u16,
        body: String,
    },
}
