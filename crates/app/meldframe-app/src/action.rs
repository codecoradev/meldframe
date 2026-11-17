//! Inputs to the app layer. They are UI-neutral: no toolkit events (standard rule U6).

use crate::state::{Panel, Tool};
use meldframe_protocol::{Command, Node};
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
    /// Make a node at a point (in the parent's space), select it and go back to the select tool. `w` and `h`
    /// give it a size in the same op (a node made by dragging a box); without them it has the default size.
    CreateNode {
        kind: CreateKind,
        x: f64,
        y: f64,
        parent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        w: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        h: Option<f64>,
    },
    /// Make a frame of a preset's size and name at a point, select it and go back to the select tool (one undo step).
    CreatePreset {
        preset: FramePreset,
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
    /// Change the size of a node. `x` and `y` move its origin in the same step: resizing from the left or the top
    /// edge changes both, and one undo takes both back.
    Resize {
        id: String,
        w: f64,
        h: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        x: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        y: Option<f64>,
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
    /// The shell loaded the file again (after `Connection::Halted`, or a replay that was too old): `nodes`
    /// as of `seq`. The replica starts from it and replays the person's unconfirmed ops on top, with the
    /// same keys; `applied` lists the keys of those ops that the server already applied, so the snapshot
    /// holds their result (leave none out: the server answers a repeated key and writes nothing, but the
    /// app would wait for an event the stream never brings). The undo history starts again. The
    /// connection goes to connecting and the app subscribes from `seq`. A bad snapshot is refused with
    /// `Event::Refused` (the `LoadError` code, or `E_SNAPSHOT_BEHIND` for a snapshot older than the app) and nothing changes. Ops that no longer apply raise `Event::OpsDropped`.
    Reload {
        #[ts(type = "number")]
        now_ms: u64,
        nodes: Vec<Node>,
        #[ts(type = "number")]
        seq: u64,
        applied: Vec<String>,
    },
}

impl Action {
    /// `CreateNode` with the default size.
    pub fn create_node(kind: CreateKind, x: f64, y: f64, parent: Option<String>) -> Self {
        Self::CreateNode {
            kind,
            x,
            y,
            parent,
            w: None,
            h: None,
        }
    }

    /// `Resize` that keeps the origin of the node.
    pub fn resize(id: impl Into<String>, w: f64, h: f64) -> Self {
        Self::Resize {
            id: id.into(),
            w,
            h,
            x: None,
            y: None,
        }
    }
}

/// A frame size to start a design from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum FramePreset {
    Desktop,
    Tablet,
    Mobile,
}

impl FramePreset {
    /// Every preset, in the order a menu lists them.
    pub const ALL: [FramePreset; 3] = [
        FramePreset::Desktop,
        FramePreset::Tablet,
        FramePreset::Mobile,
    ];

    /// Width and height in px: a 1440 x 900 laptop, an iPad portrait and a current phone.
    pub fn size(self) -> (f64, f64) {
        match self {
            FramePreset::Desktop => (1440.0, 900.0),
            FramePreset::Tablet => (768.0, 1024.0),
            FramePreset::Mobile => (390.0, 844.0),
        }
    }

    /// The name the new frame gets.
    pub fn name(self) -> &'static str {
        match self {
            FramePreset::Desktop => "Desktop",
            FramePreset::Tablet => "Tablet",
            FramePreset::Mobile => "Mobile",
        }
    }
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
    /// this way, which confirms them. `key` is the idempotency key of the op: the app recognises its own op
    /// by it. A shell that does not carry it (an old server) leaves it out, and the app compares the actor
    /// and the command instead.
    Event {
        #[ts(type = "number")]
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
    Acked {
        key: String,
        #[ts(type = "number")]
        seq: u64,
    },
    /// The server refused the op with this idempotency key. No wording: the front end writes the text.
    Rejected {
        key: String,
        code: String,
        retryable: bool,
    },
}
