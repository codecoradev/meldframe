//! Inputs to the app layer. They are UI-neutral: no toolkit events (standard rule U6).

use crate::state::{Panel, Tool};
use meldframe_protocol::Command;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Something a person did, or something the platform reports back.
///
/// Variants without data are empty structs, not unit variants: serde does not enforce `deny_unknown_fields`
/// on unit variants of an internally tagged enum.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
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
pub enum Action {
    /// Replace the selection. Ids that are not nodes are dropped.
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
    /// Hide or show the children of a layer.
    ToggleCollapsed {
        id: String,
    },
    /// Make a node at a point (in the parent's space), select it and go back to the select tool.
    CreateNode {
        kind: CreateKind,
        x: f64,
        y: f64,
        parent: Option<String>,
    },
    Move {
        id: String,
        x: f64,
        y: f64,
    },
    /// Move every selected node by the same amount, as one undo step.
    MoveBy {
        dx: f64,
        dy: f64,
    },
    Resize {
        id: String,
        w: f64,
        h: f64,
    },
    EditText {
        id: String,
        text: String,
    },
    SetProperty {
        id: String,
        property: Property,
    },
    /// Remove the nodes with their descendants, as one undo step.
    Delete {
        ids: Vec<String>,
    },
    /// Put the node right before or after a sibling; neither means in front of all siblings.
    Reorder {
        id: String,
        before: Option<String>,
        after: Option<String>,
    },
    /// Undo the actor's own last change.
    Undo {},
    /// Redo the change that was just undone.
    Redo {},
    /// Open the connection: the app asks for a subscription from its last confirmed number. `now_ms` is
    /// the shell's clock.
    Start {
        #[ts(type = "number")]
        now_ms: u64,
    },
    /// The transport is subscribed (the first time or again). `now_ms` is the shell's clock.
    Connected {
        #[ts(type = "number")]
        now_ms: u64,
    },
    /// The transport lost the connection, or an attempt to open it failed.
    Disconnected {},
    /// The backoff of `Effect::Reconnect` is over: try again.
    Reconnect {
        #[ts(type = "number")]
        now_ms: u64,
    },
    /// The shell's timer: lets the app notice a silent connection or a stuck attempt.
    Tick {
        #[ts(type = "number")]
        now_ms: u64,
    },
    /// A frame arrived from the server.
    Frame {
        #[ts(type = "number")]
        now_ms: u64,
        frame: Frame,
    },
}

/// What the node tools can make.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum CreateKind {
    Frame,
    Rectangle,
    Text,
}

/// One value of the properties panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "property", content = "value", rename_all = "camelCase")]
pub enum Property {
    Name(String),
    /// A hex colour.
    Fill(String),
    Radius(f64),
    FontSize(f64),
}

/// A message of the live connection, as the transport decodes it.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
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
pub enum Frame {
    /// Nothing to say; the connection is alive.
    Heartbeat {},
    /// The server dropped frames after `after_seq` for this client (it was too slow): subscribe again from
    /// there. `latest_seq` is the newest number the server has.
    Resync {
        #[ts(type = "number")]
        after_seq: u64,
        #[ts(type = "number")]
        latest_seq: u64,
    },
    /// An accepted op, numbered, with the actor the server stamped on it. The person's own ops come back
    /// this way, which confirms them.
    Event {
        #[ts(type = "number")]
        seq: u64,
        actor: String,
        command: Command,
    },
    /// The server refused the op with this idempotency key. No wording: the front end writes the text.
    Rejected {
        key: String,
        code: String,
        retryable: bool,
    },
}
