//! Inputs to the app layer. They are UI-neutral: no toolkit events (standard rule U6).

use crate::state::{Connection, Panel, Tool};
use meldframe_protocol::Command;
use serde::{Deserialize, Serialize};

/// Something a person did, or something the platform reports back.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
/// Variants without data are empty structs, not unit variants: serde does not enforce `deny_unknown_fields` on
/// unit variants of an internally tagged enum.
#[allow(
    clippy::large_enum_variant,
    reason = "short-lived values; boxing would leak into every consumer pattern"
)]
pub enum Action {
    /// Replace the selection.
    Select {
        ids: Vec<String>,
    },
    /// Add the node to the selection, or remove it when it is already selected.
    ToggleSelect {
        id: String,
    },
    ClearSelection {},
    SetTool {
        tool: Tool,
    },
    SetPanel {
        panel: Panel,
        open: bool,
    },
    TogglePanel {
        panel: Panel,
    },
    /// Undo the actor's own last change.
    Undo {},
    /// Redo the change that was just undone.
    Redo {},
    /// Send a change to the file.
    Submit {
        command: Command,
    },
    /// The platform reports the state of the live connection.
    ConnectionChanged {
        connection: Connection,
    },
    /// The platform reports what the actor's history can do now.
    HistoryChanged {
        can_undo: bool,
        can_redo: bool,
    },
    /// The platform reports how the server answered an [`Effect::SendCommand`](crate::Effect::SendCommand).
    CommandResult {
        op: u64,
        outcome: Outcome,
    },
}

/// How the server answered a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Outcome {
    /// The command was applied and numbered `seq` in the file's order.
    Accepted { seq: u64 },
    /// The command was refused with a stable code such as `conflict` or `invalid`. No wording: the front end
    /// writes the text (standard rule U5).
    Rejected { code: String, retryable: bool },
}
