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
    CollapsedChanged {
        id: String,
        collapsed: bool,
    },
    ConnectionChanged {
        connection: Connection,
    },
    HistoryChanged {
        can_undo: bool,
        can_redo: bool,
    },
    /// The unconfirmed ops changed: how many are left.
    PendingChanged {
        count: usize,
    },
    /// The document changed. `optimistic` is true for the person's own edit that the server has not
    /// confirmed; `actor` is then the person's own actor.
    DocumentChanged {
        actor: String,
        optimistic: bool,
    },
    /// An action could not be carried out; nothing changed. A stable code such as `not_found`.
    Refused {
        code: String,
    },
    /// The server refused one of the person's ops; the view went back. The front end words the message.
    OpRejected {
        key: String,
        code: String,
        retryable: bool,
    },
    /// The file and the events no longer fit. The front end must load the file again.
    Diverged {},
}
