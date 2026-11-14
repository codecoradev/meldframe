//! Everything a front end needs to draw the editor.

use serde::{Deserialize, Serialize};

/// The state of the editor: what is selected, which tool is active, which panels are open, how the
/// connection to the room is doing and whether undo and redo are available.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppState {
    /// Ids of the selected nodes, in the order they were selected, without duplicates.
    pub selection: Vec<String>,
    pub tool: Tool,
    pub panels: Panels,
    pub connection: Connection,
    pub history: History,
    /// Numbers of the commands sent to the server that have no answer yet, oldest first.
    pub pending: Vec<u64>,
}

/// The tool that decides what a press on the canvas does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tool {
    #[default]
    Select,
    Frame,
    Rectangle,
    Text,
}

/// A side panel of the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Panel {
    /// The nodes of the page, as the layers panel shows them.
    Left,
    /// The properties of the selection.
    Right,
}

/// Which side panels are open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Panels {
    pub left: bool,
    pub right: bool,
}

impl Default for Panels {
    fn default() -> Self {
        Self {
            left: true,
            right: true,
        }
    }
}

/// The state of the live connection to the file's room.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Connection {
    #[default]
    Connecting,
    Connected,
    Disconnected,
}

/// Whether the actor's own history has something to undo or redo. The core owns the history; this is only
/// what it last reported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct History {
    pub can_undo: bool,
    pub can_redo: bool,
}
