//! Inputs to the app layer. They are UI-neutral: no toolkit events (standard rule U6).

use crate::activity::{BatchHeader, BatchPage, BatchRef, ReviewState};
use crate::assets::AssetPage;
use crate::checkpoint::{CheckpointRequest, CheckpointRow};
use crate::export::ExportFormat;
pub use crate::frame::Frame;
use crate::layout::Direction;
pub use crate::preset::{CreateKind, FramePreset};
pub use crate::property::Property;
use crate::state::{Panel, Tool};
use crate::variables::{BindField, VariableValue};
use meldframe_protocol::{Command, ImageFit, Node, VarType, Variables};
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
    /// A node was dragged and let go with the pointer at `(x, y)` on the canvas (world space) after travelling `(dx, dy)`.
    /// One undo step. With the host flag `layout_on_canvas` on, the rules of the layout decide (`meldframe-app-rules`,
    /// `drop`): a child of an auto layout frame is put in its flow at the pointer (a reorder), or taken to the frame
    /// under the pointer; a node of a free frame dropped on an auto layout frame goes into its flow; anything else moves
    /// by `(dx, dy)`. With the flag off it is `Move` by `(dx, dy)`, whatever the pointer is over. A drop that changes
    /// nothing makes no step; an unknown node is refused (`not_found`), a value that is not a number (`invalid`).
    DropNode {
        id: String,
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
    },
    /// A selection was dragged and let go with the pointer at `(x, y)` (world space) after travelling `(dx, dy)`: the
    /// general form of `DropNode`, and what a front end sends for any drag (`ids` are the dragged nodes, the selected ones).
    /// One undo step. A node and its selected descendants are one node. With the host flag `layout_on_canvas` on: one node
    /// is `DropNode`; several move by the rules of `meldframe-app-rules`, `multi` (the flow children of one frame as a block
    /// to the slot at the pointer, the others by the drag, the flow children of several frames stay). With the flag off it
    /// is the move by `(dx, dy)` it always was: of the selection in one step when `ids` are the selection, else node by node.
    /// An id that is not a node is skipped (someone deleted it while the drag went on) and the others are dropped; when none is a node the action is refused (`not_found`) and nothing is applied; a value that is not a number is refused (`invalid`).
    DropNodes {
        ids: Vec<String>,
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
    },
    /// An arrow key was pressed with the selection (no modifier). With the host flag `layout_on_canvas` on, the selected
    /// children of one auto layout frame move one place along its main axis, as one undo step: up or down in a column,
    /// left or right in a row, towards the start or the end of the flow (`meldframe-app-rules`, `arrow`); the other
    /// two keys, a selection with no such child, and one that mixes children of different frames do nothing. With the flag
    /// off it does nothing at all: no step, no event, no effect.
    ArrowKey {
        direction: Direction,
    },
    /// Change the size of a node. `x` and `y` move its origin in the same step: resizing from the left or the top
    /// edge changes both, and one undo takes both back. With the host flag `layout_on_canvas` on, a side that is sized
    /// hug or fill and gets another size than it is drawn at becomes fixed in the same step, and the origin of a child of
    /// an auto layout frame is not written (its place is the flow's).
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
    /// Add a collection with one mode, named `default`, as one undo step. The app mints the ids (the caller sends
    /// none) and says which it made with `Event::EntityAdded`. A name that is not lowercase segments joined by `/` is
    /// refused as `E_INVALID_NAME`; the rest as any variable command is.
    CreateCollection {
        name: String,
    },
    /// Add a mode named `name` to a collection, after its last mode, with the values of mode `copy_from` (none: the
    /// default mode) in every variable, as one undo step. `E_COLLECTION_NOT_FOUND`, `E_MODE_NOT_FOUND`, and the other
    /// refusals of the variable commands.
    AddMode {
        collection: String,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        copy_from: Option<String>,
    },
    /// Remove a mode and its values, one undo step. The default (first) mode is refused (`E_DEFAULT_MODE`); a pick of
    /// the removed mode goes back to the default.
    RemoveMode {
        collection: String,
        mode: String,
    },
    /// Add a variable of this type to a collection, with the neutral value of the type (a colour: the default fill; a
    /// number: 0) in every mode, as one undo step. The id is minted by the app (`Event::EntityAdded`). `E_INVALID_NAME`,
    /// `E_DUPLICATE_NAME` (within the collection), `E_COLLECTION_NOT_FOUND`.
    AddVariable {
        collection: String,
        name: String,
        kind: VarType,
    },
    /// Remove a variable. With `detach` the fields bound to it keep the colour or number they paint in the default
    /// modes and the aliases of it keep the value they have in their own mode, and the variable goes: all one undo
    /// step. Without it a variable that is still used is refused (`E_VARIABLE_IN_USE`). A group that does not fit the
    /// queue of ops is refused whole (`outbox_full`).
    RemoveVariable {
        id: String,
        detach: bool,
    },
    /// Remove a collection. With `detach` every variable of it is let go of and removed as `RemoveVariable` does,
    /// then the collection, as one undo step; without it a collection that still holds variables is refused
    /// (`E_COLLECTION_NOT_EMPTY`).
    RemoveCollection {
        id: String,
        detach: bool,
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
    /// Open the bind picker of a field of the selected node: the variables that fit the field are listed in
    /// `AppState::picker` until it is closed. View state (no op, no undo step). Refused as `not_found` for a node that
    /// is not there, `not_selected` for a node that is not the one selected, and `not_bindable` for a field the node
    /// has no use for.
    OpenPicker {
        id: String,
        field: BindField,
    },
    /// Close the bind picker.
    ClosePicker {},
    /// Search the bind picker (the text is trimmed and cut at 200 characters).
    SetPickerQuery {
        query: String,
    },
    /// Open the Export dialog on the selected frame: the HTML of the frame is built in `AppState::export` until the
    /// dialog is closed. View state (no op, no undo step). Refused as `not_exportable` unless the one selected node
    /// is a frame (`ExportState::target`).
    OpenExport {},
    /// Change how the styles of the open dialog are written (Tailwind or plain CSS). Nothing while it is closed.
    SetExportFormat {
        format: ExportFormat,
    },
    /// Close the Export dialog.
    CloseExport {},
    /// Open the Export variables dialog: every variable of the file as CSS in `AppState::variables_io` until it is closed.
    /// View state (no op, no undo step); refused as `out_of_sync` while the app is halted.
    OpenVariablesExport {},
    /// Close the Export variables dialog.
    CloseVariablesExport {},
    /// Import the variables of CSS text (custom properties of `:root`, a dark rule, `@theme`) as one undo step, the
    /// person's own edit. It only adds: text that holds a name the file has is refused whole (`E_DUPLICATE_ID`, naming
    /// the first). Also refused whole: text of more than 50 variables (`E_IMPORT_TOO_MANY`: the browser sends one
    /// request for each op, and a longer style sheet goes through the agent tool `import_tokens`), text over 256 KiB
    /// (`E_IMPORT_TOO_LARGE`), empty text (`empty_text`) and a list that does not fit the queue (`outbox_full`).
    /// The result, report included, is `AppState::variables_io.import`; a refusal applies nothing. The result goes
    /// when the file no longer holds the variables the import added (an undo).
    ImportVariables {
        text: String,
    },
    /// Forget the result of the last import (the dialog closed).
    CloseVariablesImport {},
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
    /// The answer of `Effect::FetchCheckpoints`: it replaces the list held.
    CheckpointsLoaded {
        checkpoints: Vec<CheckpointRow>,
        can_create: bool,
    },
    /// A checkpoint request failed, was refused or got no answer in time: `Event::Refused`.
    CheckpointRequestFailed {
        request: CheckpointRequest,
        code: String,
    },
    /// The Activity tab was opened: ask for the newest page of batches (heal rule (c)).
    OpenActivity {},
    /// Fetch the page of batches older than the oldest held (when `AppState::activity.has_older`).
    LoadOlderBatches {},
    /// Show a batch: select its nodes on the canvas and mark the row (the row's `inEffect` is then worked out).
    /// Refused with `unknown_batch` for a batch the app does not hold.
    SelectBatch {
        batch: BatchRef,
    },
    /// Ask the server to set the review state of a batch (`Effect::PutBatchReview`). The row changes only when the
    /// server answers (`Action::BatchReviewed`), never at once. Refused with `unknown_batch`, or `no_batch_number` for a row with no header.
    SetBatchReview {
        batch: BatchRef,
        state: ReviewState,
    },
    /// A page of headers arrived (the answer of `Effect::FetchBatches`).
    BatchesLoaded {
        page: BatchPage,
    },
    /// The answer of the review route: the row after the change, as a header. It sets the row; it carries no count and
    /// no `can_review`, so it cannot change them (the `batch` frame carries the count), and it is not a page: it never
    /// moves heal rule (d).
    BatchReviewed {
        batch: BatchHeader,
    },
    /// A fetch or a review put failed or was refused (`forbidden`, `unavailable`, ...): raised as `Event::Refused`.
    BatchRequestFailed {
        code: String,
    },
    /// Open the image picker for a node's fill and read the workspace's assets (`Effect::FetchAssets`) into
    /// `AppState::assets` until it closes. View state. Refused as `not_found`, `not_selected` or `image_on_text`.
    OpenAssets {
        id: String,
    },
    /// Close the image picker and forget the list it read.
    CloseAssets {},
    /// Ask for the page after `AppState::assets.next`. Nothing while closed, loading or at the end.
    LoadMoreAssets {},
    /// Try the last request again after it failed (`AppState::assets.error` is set): the page after what is held, or the
    /// first page when nothing was read. The app decides which, so a front end has one button. Nothing otherwise.
    RetryAssets {},
    /// A page arrived: the answer of `Effect::FetchAssets` with its `request` and `after` (none: the first page, which replaces
    /// the list). Taken only while the picker is open and `request` is the latest. A page that is not valid
    /// (`AssetPage::is_valid`) or does not follow what is held fails the request with the code `invalid`.
    AssetsLoaded {
        request: u32,
        after: Option<String>,
        page: AssetPage,
    },
    /// A fetch failed (`unavailable`, ...), with its `request`: the code is kept in `AppState::assets.error`. Ignored as a page is.
    AssetsRequestFailed {
        request: u32,
        code: String,
    },
    /// Set the fill of a node to the image `asset_id` with `fit` as one undo step (the gradient and colour under it stay).
    /// Without `fit` the node keeps the fit it has, and cover is the fit of a node with no image. The server decides
    /// whether the asset exists, so a well-formed id is taken; another is `invalid_asset_id`, text `image_on_text`, an
    /// unknown node `not_found`. The same image and fit make no step. Closes the picker.
    SetImageFill {
        id: String,
        asset_id: String,
        #[serde(default)]
        #[ts(optional)]
        fit: Option<ImageFit>,
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
