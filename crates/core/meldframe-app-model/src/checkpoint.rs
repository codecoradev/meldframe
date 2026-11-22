//! Checkpoints as plain data (E5.3, ADR-0023): the row the server keeps, the row as the app holds it, and what a
//! failed request was. The rules (the label, the roll, the order, what makes a row valid) are in
//! `meldframe-checkpoints` and `meldframe-app-rules`; this file only holds data.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Why a checkpoint exists, spelled as the server spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum CheckpointKind {
    /// The server's own, when an agent's run started.
    RunStart,
    /// The person's, before a revert of a batch.
    BeforeRevert,
    /// The person's, before a restore.
    BeforeRestore,
    /// The person made it, with a label.
    Manual,
}

/// What the app knows of the server's checkpoints: whether it has read the list, and whether the server keeps any.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum CheckpointsStatus {
    /// The list has not been read yet (nothing is known, nothing is allowed).
    #[default]
    Unread,
    /// The list was read: `AppState::can_create_checkpoint` says whether this caller may make one.
    Loaded,
    /// The server answered the list route with not found: it keeps no checkpoints.
    Absent,
}

/// A row of the list route, of the answer to a create or the body of a `checkpoint` frame, as it came off the wire:
/// nothing in it is trusted, and the app layer checks it again before a row is made from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckpointRow {
    pub n: u32,
    #[ts(type = "number")]
    pub seq: u64,
    pub kind: CheckpointKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub about: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub by: Option<String>,
    #[ts(type = "number")]
    pub at_ms: u64,
}

/// A checkpoint of the file as the server keeps it: the file as the server had it after op `seq`, never a copy of it.
/// `id` is the text of `n`, the same in every editor. The person's own pending ops are not in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub id: String,
    pub n: u32,
    #[ts(type = "number")]
    pub seq: u64,
    pub kind: CheckpointKind,
    /// The person's text, only on a `Manual` one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub label: Option<String>,
    /// The actor the checkpoint is about: the agent whose run started, the actor of the batch being reverted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub about: Option<String>,
    /// The actor that asked for it (none for a run start, which the server takes by its own rule).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub by: Option<String>,
    #[ts(type = "number")]
    pub at_ms: u64,
}

/// The request of a checkpoint that failed or was refused (`Action::CheckpointRequestFailed`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CheckpointRequest {
    /// `Effect::FetchCheckpoints`.
    List {},
    /// `Effect::PostCheckpoint` with this key.
    Post { key: String },
    /// `Effect::FetchCheckpointState` for this number.
    State { n: u32 },
}
