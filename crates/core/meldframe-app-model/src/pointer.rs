//! The pointer on the canvas as an input of the app layer (E8.1.4, part 2): a press, a move, a release, a cancel and a leave at a
//! screen point, with the modifier keys. The app layer keeps the state machine (`meldframe_app::pointer_machine`), so a second
//! front end and a headless test select, drag, pan and hover with the same rules. A front end sends what the platform reports
//! and reads `AppState::pointer` (the hover, the cursor and the gesture that is going on); what it draws for a gesture comes from
//! `App::pointer_preview`. The camera (`AppState::camera`) maps the screen to the world, so a front end never converts a point.

use crate::gesture::{Handle, Point};
use crate::key::Modifiers;
use crate::preset::CreateKind;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What the pointer did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum PointerPhase {
    /// A button went down (or a finger touched).
    Down,
    /// The pointer moved, with a button down or not.
    Move,
    /// The button went up.
    Up,
    /// The platform took the pointer away (`pointercancel`): a gesture that is going on is abandoned.
    Cancel,
    /// The pointer left the canvas: the hover goes, and a gesture that is going on is abandoned, as with `Cancel`.
    Leave,
}

/// The button of a press. Only the primary and the middle button do anything.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum PointerButton {
    /// The left button, a touch or a pen contact.
    #[default]
    Primary,
    /// The wheel button: it pans in any tool.
    Middle,
    /// The right button: the press does nothing (the front end owns the context menu).
    Secondary,
}

/// A pointer event (`Action::Pointer`). `(x, y)` is a screen point in CSS pixels from the top left corner of the canvas. A point
/// that is not a number is ignored: nothing changes. The front end sends the events of one pointer: a second finger is a pinch
/// (`Action::Camera`, `CameraOp::Pinch`) and the front end sends `Cancel` first. The JSON may leave out everything but `phase`,
/// `x` and `y`.
///
/// What a press does is decided in the app layer: a press on a node selects it, a drag moves the selection (one undo step, by
/// `Action::DropNodes` on release), a press on empty canvas clears the selection, the middle button or the primary button with
/// `space` held pans, and a press while a text edit is open is dropped: the front end lets the field commit (its blur) before it
/// sends `Down`, and the machine ignores a press that still finds the edit open. While the app is halted a press still selects
/// and pans, and nothing that writes begins.
///
/// A double click is two clicks (a press let go without a drag) on nodes, at most 500 ms and 4 CSS pixels apart, with the
/// select tool, by `at_ms`; it begins a text edit on the node (`TextEditOp::Begin`, which does nothing for a node that is not a
/// text), as the browser's `dblclick` did. A drag, a cancel, a leave and a third click start the count again.
///
/// What the front end keeps: the pointer capture, because `Leave` abandons a drag that goes on (the file is left as it was);
/// `space` is read on each event, so `space: false` has to be sent to let go of it; and the cursor and the hover are as of the
/// last pointer event, except that a change of the tool or of the selection, and the removal of the hovered node, are seen at
/// once (`AppState::pointer`), and any other change of the file shows at the next `Move`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PointerInput {
    pub phase: PointerPhase,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub modifiers: Modifiers,
    #[serde(default)]
    pub button: PointerButton,
    /// A finger: a drag that starts on empty canvas pans (a mouse has nothing to drag there).
    #[serde(default)]
    pub touch: bool,
    /// Space is held: the primary button pans. The front end keeps the key (`KeyInput` says it keeps Space).
    #[serde(default)]
    pub space: bool,
    /// The front end's clock in milliseconds at the event: the app layer reads none, and a double click is two clicks close in
    /// time. Left out, no double click is recognised (the front end can send `Action::TextEdit` itself).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub at_ms: Option<u64>,
}

/// The cursor the canvas shows, as the CSS name. A closed set: no text from a file reaches it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "kebab-case")]
pub enum PointerCursor {
    #[default]
    Default,
    Grab,
    Grabbing,
    Crosshair,
    NwseResize,
    NeswResize,
    NsResize,
    EwResize,
}

/// What a press that became a drag is doing (`PointerState::gesture`). A press that has not moved past the threshold (3 CSS
/// pixels) is `Idle`: nothing is going on yet. A press on a handle of the selection box resizes, a press with a drawing tool draws,
/// a press on a node moves, and a press on empty canvas draws a marquee.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PointerGesture {
    #[default]
    Idle,
    /// The camera follows the pointer (`Action::Camera`).
    Pan,
    /// The selection is being dragged: `ids` are the dragged nodes, `at` is where the pointer is and `by` how far it travelled
    /// since the press, both in world space. The file changes when it is let go (`Action::DropNodes`); `App::pointer_preview` says
    /// where the nodes would land.
    Move {
        ids: Vec<String>,
        at: Point,
        by: Point,
    },
    /// A rubber band from the press on empty canvas `from` to the pointer `to` (world points, not normalised). On release the
    /// top-level nodes it overlaps are selected (`Action::Select`), or added to the selection with Shift held.
    Marquee { from: Point, to: Point },
    /// A handle of the one selected node is being dragged by `by` (world units since the press): the box it would leave is
    /// `App::pointer_preview` (`App::resize_preview`), and the file changes on release (`GestureOp::ResizeHandle`).
    Resize {
        id: String,
        handle: Handle,
        by: Point,
    },
    /// A node of `kind` is being drawn from the press `from` to the pointer `to` (world points): `App::pointer_preview`
    /// (`App::draw_preview`) says the box, and the node is made on release (`GestureOp::Draw`, a click makes the default size).
    Draw {
        kind: CreateKind,
        from: Point,
        to: Point,
    },
}

/// What a front end reads to draw the pointer (`AppState::pointer`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PointerState {
    /// The node under the pointer while nothing is going on and the select tool is active: an id of the file, never text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hover: Option<String>,
    pub cursor: PointerCursor,
    pub gesture: PointerGesture,
}

impl PointerInput {
    /// The event that abandons the gesture that is going on (`PointerPhase::Cancel`; the point is not read): what the Escape key
    /// makes while one goes.
    pub fn cancel() -> Self {
        PointerInput {
            phase: PointerPhase::Cancel,
            x: 0.0,
            y: 0.0,
            modifiers: Modifiers::default(),
            button: PointerButton::Primary,
            touch: false,
            space: false,
            at_ms: None,
        }
    }
}
