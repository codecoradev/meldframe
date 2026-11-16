//! Inputs to the app layer. They are UI-neutral: no toolkit events (standard rule U6).

use crate::state::{Panel, Tool};
use meldframe_protocol::{Command, Node};
use serde::{Deserialize, Serialize};

/// Something a person did, or something the platform reports back.
///
/// Variants without data are empty structs, not unit variants: serde does not enforce `deny_unknown_fields`
/// on unit variants of an internally tagged enum.
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
        now_ms: u64,
    },
    /// The transport is subscribed (the first time or again). `now_ms` is the shell's clock.
    Connected {
        now_ms: u64,
    },
    /// The transport lost the connection, or an attempt to open it failed.
    Disconnected {},
    /// The backoff of `Effect::Reconnect` is over: try again.
    Reconnect {
        now_ms: u64,
    },
    /// The shell's timer: lets the app notice a silent connection or a stuck attempt.
    Tick {
        now_ms: u64,
    },
    /// A frame arrived from the server.
    Frame {
        now_ms: u64,
        frame: Frame,
    },
    /// The shell loaded the file again (after `Connection::Halted`, or a replay that was too old): `nodes`
    /// as of `seq`. The replica starts from it and replays the person's unconfirmed ops on top, with the
    /// same keys; `applied` lists the keys of those ops that the server already applied, so the snapshot
    /// holds their result (leave none out: the server answers a repeated key and writes nothing, but the
    /// app would wait for an event the stream never brings). The undo history starts again. The
    /// connection goes to connecting and the app subscribes from `seq`. A bad snapshot is refused with
    /// `Event::Refused` (the `LoadError` code, or `E_SNAPSHOT_BEHIND` for a snapshot older than the app) and nothing changes. Ops that no longer apply raise `Event::OpsDropped`.
    Reload {
        now_ms: u64,
        nodes: Vec<Node>,
        seq: u64,
        applied: Vec<String>,
    },
}

/// What the node tools can make.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CreateKind {
    Frame,
    Rectangle,
    Text,
}

/// One value of the properties panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "property", content = "value", rename_all = "camelCase")]
pub enum Property {
    Name(String),
    /// A hex colour.
    Fill(String),
    Radius(f64),
    FontSize(f64),
}

/// A message of the live connection, as the transport decodes it.
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
pub enum Frame {
    /// Nothing to say; the connection is alive.
    Heartbeat {},
    /// The server dropped frames after `after_seq` for this client (it was too slow): subscribe again from
    /// there. `latest_seq` is the newest number the server has.
    Resync { after_seq: u64, latest_seq: u64 },
    /// An accepted op, numbered, with the actor the server stamped on it. The person's own ops come back
    /// this way, which confirms them. `key` is the idempotency key of the op: the app recognises its own op
    /// by it. A shell that does not carry it (an old server) leaves it out, and the app compares the actor
    /// and the command instead.
    Event {
        seq: u64,
        actor: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        command: Command,
    },
    /// The server answered the send of the op with this key: it was applied as event `seq` (for a repeated
    /// key, the first time, and nothing was written). When the app already follows the file up to `seq`
    /// (a reload went past it) the op is done; otherwise its event is still to come and this changes
    /// nothing. Contract of the shell: after a reload it sends the unconfirmed ops again and must report
    /// every answer this way, including the one for a repeated key; until it does, an op the snapshot already
    /// holds stays pending and counts against the cap. The shell side is not built yet: only the test harness
    /// of `meldframe-app-tests` emits this frame.
    Acked { key: String, seq: u64 },
    /// The server refused the op with this idempotency key. No wording: the front end writes the text.
    Rejected {
        key: String,
        code: String,
        retryable: bool,
    },
}
