//! Inputs to the app layer. They are UI-neutral: no toolkit events (standard rule U6).

use crate::state::{Panel, Tool};
use crate::variables::{BindField, VariableValue};
use meldframe_protocol::{Command, Node, Variables};
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
    /// Copy a frame and its subtree beside the original as a variant, with new ids, and select the copy (one undo
    /// step).
    MakeVariant {
        id: String,
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
    /// Pick the mode a collection is shown in (`None`: its default mode). It is view state: nothing is written to
    /// the file, no op is sent and the undo history does not change; the canvas paints the resolved values
    /// again. An unknown collection or mode is refused (`E_COLLECTION_NOT_FOUND`, `E_MODE_NOT_FOUND`).
    SetMode {
        collection: String,
        mode: Option<String>,
    },
    /// Apply variable commands (collections, modes, variables, values) as one undo step. They are checked one
    /// after the other before any is applied, so a list that would fail half way applies nothing; the refusal is
    /// the `E_` code of the first command that fails. A command that is not about variables is refused
    /// (`not_a_variable_command`).
    EditVariables {
        commands: Vec<Command>,
    },
    /// Search the Variables panel (view state: nothing is written to the file). The text is trimmed and cut at 200
    /// characters; every word has to match (see `VariablesPanel::query`).
    SetVariablesQuery {
        query: String,
    },
    /// Set the value of a variable in one mode (one undo step). A literal is read by the type of the variable: a
    /// `#hex` colour or a number (text that is not a number is refused as `invalid_value`); the rest is checked as
    /// any variable command is, and refused with its `E_` code. A value equal to the one it has makes no step.
    SetVariableValue {
        variable: String,
        mode: String,
        value: VariableValue,
    },
    /// Bind a field of a node to a variable (`Some`), or unbind it (`None`): the field then holds the value that
    /// is painted in the modes on view, as a literal. One undo step. A field the node kind has no use for is
    /// refused as `not_bindable`; a variable of the wrong type or a missing one with its `E_` code.
    BindProperty {
        id: String,
        field: BindField,
        variable: Option<String>,
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
    /// Revert an agent's batch (E5.2): the ops of `actor` whose keys are `{key_prefix}:{n}`. Only the fields
    /// whose last writer is still that batch go back; the change is the person's own and one undo takes it
    /// back. Refused with `unknown_batch` (no such ops since the app loaded the file), `nothing_to_revert` or
    /// `log_mismatch`. A checkpoint is taken first.
    RevertBatch {
        actor: String,
        key_prefix: String,
    },
    /// Make a checkpoint at the confirmed state of the file (E5.3). A label over 128 characters is refused as
    /// `limit`.
    CreateCheckpoint {
        label: String,
    },
    /// Bring the file back to a checkpoint as new ops of the person (one undo step); nothing in the log is
    /// rewritten. A checkpoint is taken first. Refused with `unknown_checkpoint`, `not_in_log` (the app has
    /// dropped the ops back to it), `nothing_to_restore` or `log_mismatch`.
    RestoreCheckpoint {
        id: String,
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
        /// The collections and variables of the snapshot (empty for a file without any; the JSON may leave the field out).
        #[serde(default)]
        #[ts(as = "Variables")]
        variables: Variables,
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
    /// 0 (invisible) to 1; a value outside that range is refused. A bound opacity becomes the number.
    Opacity(f64),
    /// Whether a frame cuts off what is drawn outside it.
    Clip(bool),
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
