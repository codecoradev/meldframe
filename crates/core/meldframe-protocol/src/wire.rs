//! Messages on the live connection between a client (or agent) and the server.

use crate::command::Command;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Most `deps` one op may name. The server refuses more with the code `invalid`.
pub const MAX_OP_DEPS: usize = 512;

/// A message from a client (or an agent) to the server.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export)]
#[allow(
    clippy::large_enum_variant,
    reason = "short-lived wire values; boxing would leak into every consumer pattern"
)]
pub enum ClientMsg {
    Subscribe {
        id: String,
        room: String,
        /// Replay accepted ops after this sequence number (a reconnecting client sends its last one).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional, type = "number")]
        since_seq: Option<u64>,
    },
    Unsubscribe {
        room: String,
    },
    /// `if_seq` (optional): optimistic precondition. The op is refused with the code `conflict` when a node this
    /// command touches was changed by another op with a sequence number above `if_seq`.
    Op {
        id: String,
        idempotency_key: String,
        room: String,
        command: Command,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional, type = "number")]
        if_seq: Option<u64>,
        /// `deps` (optional): idempotency keys of ops by the SAME actor that were still unconfirmed when this op was
        /// made and are its premise (for example an undo computed from a view that contains them). The op is accepted
        /// only if EVERY dep was applied by the server; otherwise the server answers `dependency_rejected` and writes
        /// nothing. A dep that failed only TEMPORARILY on the same connection makes the dependent op answer with the
        /// retryable `dependency_pending` (reconnect and resend in order). Empty = no dependencies; a client that
        /// omits the field stays valid.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[ts(optional, type = "Array<string>")]
        deps: Vec<String>,
    },
    /// The client advertises capabilities (for example `["capture"]`) for a room it already subscribed to on this
    /// connection. A separate message, not a field on `Subscribe`, so older clients stay valid. A client that
    /// advertises `capture` must answer every [`ServerMsg::CaptureRequest`].
    Capabilities {
        room: String,
        capabilities: Vec<String>,
    },
    /// The client cannot or will not fulfil a `CaptureRequest` (codes: `node_not_found`, `too_large`, `stale_client`,
    /// `render_failed`). Only the client the request was sent to is heard.
    CaptureFailed {
        request_id: String,
        code: String,
        message: String,
    },
    /// An empty struct variant, not a unit variant: serde does not enforce `deny_unknown_fields` on unit variants of
    /// an internally tagged enum, so `{"type":"ping","x":1}` would pass.
    Ping {},
}

/// A message from the server to a client.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export)]
#[allow(
    clippy::large_enum_variant,
    reason = "short-lived wire values; boxing would leak into every consumer pattern"
)]
pub enum ServerMsg {
    Connected {
        connection_id: String,
    },
    Subscribed {
        id: String,
        room: String,
    },
    Ack {
        id: String,
        #[ts(type = "number")]
        seq: u64,
    },
    /// An accepted op, numbered and replayable.
    Event {
        room: String,
        #[ts(type = "number")]
        seq: u64,
        actor: String,
        command: Command,
    },
    /// The requested replay is older than the kept log: the client must reload from a snapshot.
    ReplayOverflow {
        room: String,
        #[ts(type = "number")]
        latest_seq: u64,
    },
    Error {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        id: Option<String>,
        message: String,
        retryable: bool,
        /// Machine code (optional): `conflict` (`ifSeq` failed), `stale` (`ifSeq` older than the kept log), `invalid`
        /// (semantic validation), `internal`. An older client may ignore it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        code: Option<String>,
        /// For `quota_exceeded` and other retryable errors: wait this many milliseconds before trying again.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional, type = "number")]
        retry_after_ms: Option<u64>,
    },
    Pong {},
    /// Ephemeral: asks ONE client for an image of a node. The client renders offscreen and then
    /// `POST /captures/{requestId}` (PNG, `Authorization: Bearer <uploadToken>`, header `X-Capture-Seq`). An empty
    /// `node_id` means the whole page. `min_seq`: the PNG must reflect the file at a sequence number of at least this
    /// value (the client waits for events that have not arrived yet).
    CaptureRequest {
        request_id: String,
        room: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        node_id: Option<String>,
        scale: f64,
        #[ts(type = "number")]
        max_pixels: u64,
        #[ts(type = "number")]
        min_seq: u64,
        upload_token: String,
        #[ts(type = "number")]
        timeout_ms: u64,
    },
}
