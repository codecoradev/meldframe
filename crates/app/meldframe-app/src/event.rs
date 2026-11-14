//! Changes of [`AppState`](crate::AppState), one per change, for a front end that redraws only what moved.

use crate::state::{Connection, Panel, Tool};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Event {
    SelectionChanged {
        ids: Vec<String>,
    },
    ToolChanged {
        tool: Tool,
    },
    PanelChanged {
        panel: Panel,
        open: bool,
    },
    ConnectionChanged {
        connection: Connection,
    },
    HistoryChanged {
        can_undo: bool,
        can_redo: bool,
    },
    /// The server refused a command. The front end words the message from `code`.
    CommandRejected {
        op: u64,
        code: String,
        retryable: bool,
    },
}
