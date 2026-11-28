//! Everything a front end needs to draw the editor.

use crate::activity::ActivityState;
use crate::assets::AssetsState;
use crate::camera::{Camera, Viewport};
use crate::checkpoint::{Checkpoint, CheckpointsStatus};
use crate::confirm::{Confirm, RightTab};
use crate::export::ExportState;
use crate::layers::LayerRow;
use crate::layout::{LayoutNote, Sides};
use crate::limits::AppLimits;
use crate::manage::ManageState;
use crate::overlay::Overlay;
use crate::pointer::PointerState;
use crate::text_edit::TextEdit;
use crate::variables::{BindField, Picker, VariablesPanel};
use crate::variables_io::VariablesIo;
use meldframe_protocol::{
    Align, Border, Gradient, ImageFill, Justify, LayoutMode, NodeKind, Shadow, SizeMode, TextAlign,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// The state of the editor: what is selected, which tool is active, which panels are open, how the
/// connection to the room is doing and whether undo and redo are available. The document itself is read
/// with `App::nodes`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
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
    #[ts(type = "number")]
    pub confirmed_seq: u64,
    pub history: History,
    /// Idempotency keys of the person's ops the server has not confirmed yet, oldest first.
    pub pending: Vec<String>,
    /// For the layers panel: the actor of the last change to each node, as the server stamped it, made safe to show
    /// and to key by (rule U9: strict profile, at most 128 bytes).
    pub last_actor: BTreeMap<String, String>,
    /// The Layers panel: its rows with every name made safe to show (rule U9). Filled while `panels.layers` is open,
    /// empty otherwise.
    #[serde(default)]
    pub layers: Vec<LayerRow>,
    /// The checkpoints of the file as the server last said (E5.3, ADR-0023), newest first: a person's, the one the
    /// server takes when an agent's run starts, and the ones before a revert or a restore. The app never adds one
    /// itself: a row is here when the server has said so.
    pub checkpoints: Vec<Checkpoint>,
    /// Whether this caller may make a checkpoint (the list route's `canCreate`). False until the list is read, and
    /// where the server keeps none.
    #[serde(default)]
    pub can_create_checkpoint: bool,
    /// Whether the list was read and whether the server keeps any, so the panel can say why nothing can be made.
    #[serde(default)]
    pub checkpoints_status: CheckpointsStatus,
    /// The mode picked for each collection that is not on its default mode (collection id to mode id). View
    /// state: it decides what the canvas paints and is never written to the file.
    pub modes: BTreeMap<String, String>,
    /// The Variables panel (E3.4): filled while `panels.variables` is open, empty otherwise.
    pub variables: VariablesPanel,
    /// The limits and tables a front end shows (the import cap, the export pixel limit and scales, the frame presets): the
    /// rule's own constants, never copied into a front end. The manage view's limits are `manage.limits`.
    #[serde(default)]
    pub limits: AppLimits,
    /// The Variables manage view: the collection shown, the question before a removal, the rename field and the forms.
    #[serde(default)]
    pub manage: ManageState,
    /// The Activity panel (E5.1): lanes, batches with their notes and review states, and touch rings.
    pub activity: ActivityState,
    /// The tab of the right card (`Action::Activity` with `ActivityOp::SetTab`).
    #[serde(default)]
    pub right_tab: RightTab,
    /// The question the person is being asked in the Activity panel: revert a batch or restore a checkpoint
    /// (`Action::Activity`: `AskRevert`, `AskRestore`, `Cancel` and the confirmations).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub confirm: Option<Confirm>,
    /// The overlay of the layout that is open, the shortcut sheet or the command palette (`Action::Overlay`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub overlay: Option<Overlay>,
    /// Export (D9): the frame an export acts on, and the dialog while it is open.
    #[serde(default)]
    pub export: ExportState,
    /// The Export variables and Import variables dialogs (D8).
    #[serde(default)]
    pub variables_io: VariablesIo,
    /// The bind picker of a field of the selected node: `Some` only while it is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub picker: Option<Picker>,
    /// The image picker of the Fill section: the node it chooses for while it is open, and the assets of the workspace
    /// read so far.
    #[serde(default)]
    pub assets: AssetsState,
    /// The text being edited: `Some` only while a text node is open for editing (`Action::TextEdit`). The field's own draft
    /// is not here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text_edit: Option<TextEdit>,
    /// The camera of the canvas (`Action::Camera`): pan and zoom. View state: it is never written to the file.
    #[serde(default)]
    pub camera: Camera,
    /// The size of the canvas in CSS pixels, as the front end reported it.
    #[serde(default)]
    pub viewport: Viewport,
    /// The pointer (`Action::Pointer`): the node under it, the cursor and the gesture that is going on. View state.
    #[serde(default)]
    pub pointer: PointerState,
}

/// The tool that decides what a press on the canvas does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Tool {
    #[default]
    Select,
    Frame,
    Rectangle,
    Ellipse,
    Text,
}

/// A side panel of the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Panel {
    /// The nodes of the page.
    Layers,
    /// The properties of the selection.
    Properties,
    /// The collections and variables of the file.
    Variables,
}

/// Which side panels are open, and which layers are collapsed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Panels {
    pub layers: bool,
    pub properties: bool,
    /// The Variables panel; open at first load, as the Inspector board draws it.
    pub variables: bool,
    /// Ids of the nodes whose children the layers panel hides, in the order they were collapsed.
    pub collapsed: Vec<String>,
}

impl Default for Panels {
    fn default() -> Self {
        Self {
            layers: true,
            properties: true,
            variables: true,
            collapsed: Vec::new(),
        }
    }
}

/// The state of the live connection to the file's room.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
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
    /// The file and the server's events no longer fit (`Event::Diverged`). Only `Action::Reload` with a
    /// fresh snapshot (or a new `App`) goes on; selection, tool and panels still work.
    Halted,
}

/// What the actor's own history can do. No wording: the front end writes the button text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Properties {
    pub id: String,
    /// The node's name made safe to show (display profile, `meldframe_text::label`); the file keeps what was written.
    pub name: String,
    pub kind: NodeKind,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// The colour that is painted: the literal, or what the bound variable has in the picked modes.
    pub fill: String,
    /// The type of the fill the person sees: an image over everything, else a gradient, else the colour (`none` for
    /// the word none).
    pub fill_kind: FillKind,
    /// Whether the node's fill can be a gradient (the canvas paints none on text), and the fewest and most stops of one.
    pub gradient_allowed: bool,
    /// Whether the node can be given an image (the kinds but text: the core refuses `image_on_text`), so the Fill section
    /// offers the Image type.
    pub image_allowed: bool,
    pub min_stops: usize,
    pub max_stops: usize,
    /// The gradient over `fill`, as the file holds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub gradient: Option<Gradient>,
    /// The image over `fill` and the gradient. It is picked from the workspace's assets (`Action::OpenAssets`,
    /// `Action::SetImageFill`) and taken away by the Solid, None or gradient type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub image: Option<ImageFill>,
    pub radius: f64,
    /// The uniform border; a width of 0 is none.
    pub border: Border,
    /// The drop shadows in the order they are drawn (the first is at the bottom), at most `max_shadows`.
    pub shadows: Vec<Shadow>,
    /// The most shadows a node holds.
    pub max_shadows: usize,
    /// How a frame lays out its children (`none` for a free frame), and the gap between them.
    pub layout: LayoutMode,
    pub gap: f64,
    /// The padding on each side, with the side's own value or the uniform one it takes when it is not set.
    pub padding: Sides,
    /// The one value the four sides of the padding share, or none when they differ (the uniform field is then mixed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub padding_uniform: Option<f64>,
    /// The margin on each side: room around the node inside an auto layout frame.
    pub margin: Sides,
    /// How the width and the height are sized: fixed, hug what is inside, or fill the parent.
    pub sw: SizeMode,
    pub sh: SizeMode,
    /// Where the children sit across the layout and along it.
    pub align: Align,
    pub justify: Justify,
    /// The node leaves the flow of an auto layout parent and sits at its own position.
    pub absolute: bool,
    /// The parent is a frame with auto layout: the margin and the fill sizing mean something there.
    pub in_layout: bool,
    /// `x` and `y` are where the auto layout of the parent puts the node, not values of the node: the panel shows them
    /// and does not take an edit. Only with the host flag `layout_on_canvas` on, and only for a child of the flow (not an
    /// absolute one).
    pub position_locked: bool,
    /// Settings that have no effect where the node is, in the order of the fields.
    pub notes: Vec<LayoutNote>,
    /// Which sections of the panel this kind of node has.
    pub sections: Sections,
    pub text: String,
    pub font_size: f64,
    /// The weight (100 to 900 in steps of 100) and the alignment of a text; none for any other kind of node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub font_weight: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text_align: Option<TextAlign>,
    /// Opacity as painted, 0 to 1 (a bound field: what the variable resolves to, held to that range).
    pub opacity: f64,
    /// Whether a frame cuts off what is drawn outside it.
    pub clip: bool,
    /// The fields bound to a variable (`fill`, `radius`, `opacity`); their value above is the resolved one.
    pub bound: Vec<String>,
    /// The same fields with the variable each one holds, for the chip a bound field shows.
    pub bindings: Vec<FieldBinding>,
}

/// Which of the optional sections of the panel a node has: the canvas draws no corners on text and an ellipse, a frame
/// alone clips and lays out its children, a text alone has the Text section, and the Margin section is there in a flow
/// and wherever the node has a margin or is absolute (a note says they have no effect where it is, and the fields that
/// clear them must not be hidden by it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sections {
    pub radius: bool,
    pub clip: bool,
    pub font_size: bool,
    pub layout: bool,
    pub placement: bool,
}

/// A field of the selected node and the variable it is bound to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldBinding {
    pub field: BindField,
    /// The id of the variable.
    pub variable: String,
    /// Its name, as the Variables panel lists it (made safe to show); the id when the variable is gone.
    pub name: String,
    /// The variable no longer exists (another actor removed it): the field keeps the binding and paints its default.
    pub missing: bool,
}

/// The type of a fill, as the type switch of the Fill section lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum FillKind {
    /// A colour (or a variable that holds one).
    Solid,
    /// No fill.
    None,
    /// A gradient along a line.
    Linear,
    /// A gradient from the centre.
    Radial,
    /// An image. Choosing it opens the image picker (`Action::OpenAssets`); the type switch itself cannot make one
    /// (`Property::FillType(Image)` is refused as `no_image` while the node has none).
    Image,
}
