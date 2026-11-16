//! Everything a front end needs to draw the editor.

use meldframe_protocol::NodeKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The state of the editor: what is selected, which tool is active, which panels are open, how the
/// connection to the room is doing and whether undo and redo are available. The document itself is read
/// with [`App::nodes`](crate::App::nodes).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppState {
    /// Ids of the selected nodes, in the order they were selected, without duplicates. Every id is a node
    /// of the document: a node that goes away leaves the selection.
    pub selection: Vec<String>,
    pub tool: Tool,
    pub panels: Panels,
    /// The properties panel: the one selected node, `None` for no selection or several.
    pub properties: Option<Properties>,
    pub connection: Connection,
    /// The last server sequence number applied to the confirmed file.
    pub confirmed_seq: u64,
    pub history: History,
    /// Idempotency keys of the person's ops the server has not confirmed yet, oldest first.
    pub pending: Vec<String>,
    /// For the layers panel: the actor of the last change to each node, as the server stamped it.
    pub last_actor: BTreeMap<String, String>,
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
    /// The nodes of the page.
    Layers,
    /// The properties of the selection.
    Properties,
}

/// Which side panels are open, and which layers are collapsed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Panels {
    pub layers: bool,
    pub properties: bool,
    /// Ids of the nodes whose children the layers panel hides, in the order they were collapsed.
    pub collapsed: Vec<String>,
}

impl Default for Panels {
    fn default() -> Self {
        Self {
            layers: true,
            properties: true,
            collapsed: Vec::new(),
        }
    }
}

/// The state of the live connection to the file's room.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Connection {
    /// Not subscribed yet.
    #[default]
    Connecting,
    /// Subscribed; events arrive.
    Live,
    /// The connection dropped and attempt number `attempt` (from 1) to restore it is under way.
    Reconnecting { attempt: u32 },
    /// The connection dropped; the next attempt waits for its backoff.
    Offline,
}

/// What the actor's own history can do. No wording: the front end writes the button text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct History {
    pub can_undo: bool,
    pub can_redo: bool,
    /// How many steps undo can go back.
    pub undo_steps: usize,
    /// How many steps redo can go forward.
    pub redo_steps: usize,
}

/// The editable values of the selected node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Properties {
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub fill: String,
    pub radius: f64,
    pub text: String,
    pub font_size: f64,
}
