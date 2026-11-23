//! Checkpoints as plain data (E5.3, ADR-0023): the row the app holds, in the shape of the row the server keeps. The
//! rules (the label, the roll, the order) are in `meldframe-app-rules`; this file only holds data.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Why a checkpoint exists, spelled as the server spells it (ADR-0023).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum CheckpointKind {
    /// Taken when another actor's run started.
    RunStart,
    /// Taken before a revert of a batch.
    BeforeRevert,
    /// Taken before a restore.
    BeforeRestore,
    /// The person made it, with a label.
    Manual,
}

/// A named point in the file's history that can be restored: the file as the server confirmed it after op `seq`, never a
/// copy of it. `id` is the text of `n`. The person's own pending ops are not in it.
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
