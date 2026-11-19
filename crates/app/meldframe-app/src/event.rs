//! Changes of [`AppState`](crate::AppState), one per change, for a front end that redraws only what moved.

use crate::state::{Connection, Panel, Tool};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
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
    /// The server refused an op for now (`retryable`); the app sends it again with the same key. Attempt
    /// number `attempt` (from 1) is under way.
    OpRetry {
        key: String,
        attempt: u32,
    },
    /// The server refused one of the person's ops for good (or the retries ran out); the view went back. The front end words the message.
    OpRejected {
        key: String,
        code: String,
        retryable: bool,
    },
    /// A reload dropped this many unconfirmed ops: they no longer apply to the new file. The front end
    /// tells the person.
    OpsDropped {
        count: usize,
    },
    /// A batch was reverted: `commands` ops were made as the person's change, and `skipped` lists what was left
    /// as it is.
    Reverted {
        commands: usize,
        skipped: Vec<RevertSkip>,
    },
    /// A checkpoint was restored with `commands` new ops.
    Restored {
        id: String,
        commands: usize,
    },
    /// A checkpoint was added (or the oldest dropped): how many there are now.
    CheckpointsChanged {
        count: usize,
    },
    /// The file and the events no longer fit. The app halts (`Connection::Halted`) and ignores frames and
    /// edits; the shell loads the file again and dispatches `Action::Reload` (or makes a new `App`).
    Diverged {},
}

/// A part of a batch that a revert left alone. `what` is a field name, `place` or `node`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevertSkip {
    pub node: String,
    pub what: String,
    pub reason: RevertSkipReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum RevertSkipReason {
    /// Someone else wrote it after the batch.
    ChangedSince,
    /// It no longer applies to the file.
    NotApplicable,
}
