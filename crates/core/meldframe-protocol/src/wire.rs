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
    /// an internally tagged enum, so `{"type":"ping","x":1}` would pass. On a live stream (`/v1/live/{file}`) a
    /// client sends one at least once within the server's `dead_after_ms`, or the stream is dropped and the
    /// socket closes: a browser cannot send a WebSocket-level ping, so this is the proof of life.
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
    /// An accepted op, numbered and replayable. `key` is the idempotency key of the op: a client that sent
    /// it recognises its own op by `key` (not by actor and command, which a second session of the same
    /// actor or a repeated command can share). Absent in an event written before the key was carried;
    /// such a client falls back to the actor and the command.
    Event {
        room: String,
        #[ts(type = "number")]
        seq: u64,
        actor: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        key: Option<String>,
        command: Command,
    },
    /// The requested replay is older than the kept log: the client must reload from a snapshot.
    ReplayOverflow {
        room: String,
        #[ts(type = "number")]
        latest_seq: u64,
    },
    /// Durable resync marker on a live stream (ADR-0009): the client was too slow (or its replay was cut), so
    /// events after `after_seq` were not sent and the stream delivers nothing more until the client
    /// subscribes again with `since_seq = after_seq` (the server never starts before the marker, and accepts a
    /// `subscribe` on a live stream only after one). `latest_seq` is the newest number of the file. Not
    /// numbered itself, and never replayed.
    Resync {
        room: String,
        #[ts(type = "number")]
        after_seq: u64,
        #[ts(type = "number")]
        latest_seq: u64,
    },
    /// Ephemeral keep-alive on a live stream: nothing happened, the connection is up. No sequence number.
    Heartbeat {},
    /// Ephemeral touch hint on a live stream: `actor` (a person or an agent) changed these nodes, the newest
    /// op being `seq`; the editor draws the actor's touch ring. Best effort: never stored, never replayed,
    /// at most one per actor and file per 100 ms (the latest `node_ids` win), at most 64 ids, and it carries
    /// no document content. It is not numbered itself: `seq` only says which op it is about.
    Touch {
        actor: String,
        node_ids: Vec<String>,
        #[ts(type = "number")]
        seq: u64,
    },
    /// Ephemeral hint on a live stream (ADR-0021): the header of a numbered batch was written or changed, as the
    /// server read it back after the change. It comes after the call's op events, has no sequence number of its own,
    /// is best effort (queued only while the queue keeps its last slots free, so it never causes a resync by itself, though it
    /// uses the slack of a stalled reader; never replayed; a drop gives no signal) and
    /// is a hint over the log: `GET /v1/files/{file}/batches` is the truth, and `rev` (one counter per file, one
    /// step for each change of a header) lets a client keep the newest row of each `n` and see a gap.
    /// `unreviewed_count` is the number of headers of the file that are not `accepted`, as of this change. `note` is
    /// plain text. `review` is `unreviewed`, `accepted` or `needs_review`.
    Batch {
        room: String,
        n: u32,
        #[ts(type = "number")]
        rev: u64,
        actor: String,
        key_prefix: String,
        tool: String,
        commands: u32,
        written: u32,
        atomic: bool,
        #[ts(type = "number")]
        first_seq: u64,
        #[ts(type = "number")]
        last_seq: u64,
        #[ts(type = "number")]
        at_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        note: Option<String>,
        review: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        reviewed_by: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional, type = "number")]
        reviewed_at_ms: Option<u64>,
        unreviewed_count: u32,
    },
    /// Ephemeral hint on a live stream (ADR-0023): a checkpoint of the file was made, as the server read it back. It
    /// comes after the call's op events, has no sequence number of its own, is best effort (queued only while the
    /// queue keeps its last slots free, so it never causes a resync by itself; never replayed; a drop gives no signal)
    /// and is a hint over the rows: `GET /v1/files/{file}/checkpoints` is the truth. A row never changes, so the frame
    /// needs no `rev`; a client that sees `n` more than 1 above the highest it holds reads the list. A roll of old
    /// rows is not announced. `seq` is a number in the file's log (the file as it was after op `seq`). `kind` is
    /// `runStart`, `beforeRevert`, `beforeRestore` or `manual`. `label` is plain text, `about` the actor the checkpoint
    /// is about and `by` the actor that asked for it (none for a run start).
    Checkpoint {
        room: String,
        n: u32,
        #[ts(type = "number")]
        seq: u64,
        kind: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        about: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        by: Option<String>,
        #[ts(type = "number")]
        at_ms: u64,
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
    /// `POST /captures/{requestId}` with the PNG as the body, its own `Authorization: Bearer <token>` (the
    /// principal's), the header `X-Capture-Token: <uploadToken>` and the header `X-Capture-Seq` (the sequence number
    /// it rendered at); or it answers with `ClientMsg::CaptureFailed`. An empty
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
