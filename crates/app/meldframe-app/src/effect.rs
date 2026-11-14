//! Work only the platform can do. The front end carries it out and reports the result as an action.

use meldframe_protocol::Command;
use serde::{Deserialize, Serialize};

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
    /// Send the command to the server as an op; answer with `Action::CommandResult` carrying the same `op`.
    SendCommand { op: u64, command: Command },
    /// Ask the core to undo the actor's own last change; the new history comes back as `Action::HistoryChanged`.
    RequestUndo {},
    /// Ask the core to redo; the new history comes back as `Action::HistoryChanged`.
    RequestRedo {},
    /// Open the connection again; the platform decides when.
    Reconnect {},
}
