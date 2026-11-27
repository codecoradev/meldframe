//! A drag on the canvas as an intent in world space (audit finding T8, issue 324, decision 43 clause Q3): draw a node with a
//! tool, or move one handle of the selection box. The front end maps the pointer through its camera and sends world points;
//! the app layer never sees a screen point. The geometry (the normalised box, the smallest size, the frame the new node lands
//! in, the edge that stays when a handle moves, the sides that become fixed) is decided here once, so a second front end
//! does not write it again. What stays in the front end: the camera, the threshold that turns a press into a drag and the
//! pointer modes (decision 43, Q9).

use crate::key::Modifiers;
use crate::preset::CreateKind;
pub use meldframe_camera::Point;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One of the eight handles of a selection box, named by the compass point it sits at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum Handle {
    Nw,
    N,
    Ne,
    E,
    Se,
    S,
    Sw,
    W,
}

impl Handle {
    /// Whether the handle holds the left edge, the right edge, the top edge and the bottom edge.
    pub fn edges(self) -> (bool, bool, bool, bool) {
        use Handle::*;
        (
            matches!(self, Nw | W | Sw),
            matches!(self, Ne | E | Se),
            matches!(self, Nw | N | Ne),
            matches!(self, Sw | S | Se),
        )
    }
}

/// What a person does with the pointer on the canvas that makes or sizes a node (`Action::Gesture`). One undo step each.
/// `mods` are the keys held (`Modifiers`, the type of a key press). They are part of the intent so that a rule that needs one (a
/// square with Shift, a resize from the centre with Alt) has it without a new action; no modifier changes the geometry today,
/// because the web editor never used one for a draw or a resize and these rules are that editor's, moved. The JSON may leave
/// `mods` out (no key held), but the generated TypeScript type has it required (`#[ts(optional)]` is for `Option` fields only),
/// so a TypeScript caller sends `mods: {}` when no key is held: every key of `Modifiers` defaults to false.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum GestureOp {
    /// The pointer was pressed at `from` with a node tool and let go at `to`: make a node of `kind` in the box between the
    /// two points, select it and go back to the select tool. The box is normalised (a drag to the left or up gives the
    /// same box) and a side that was dragged flat gets the smallest size, 1. `from` equal to `to` is a click: the node
    /// has the default size of its kind with its top left corner at the point. The node goes into the front-most frame
    /// under `from` (the node under `from` if it is a frame, else the nearest frame above it), else onto the canvas, and
    /// its position is worked out in that frame's space from where the canvas draws the frame. A value that is not a number
    /// is refused (`invalid`), and one outside the limits of the file (`invalid` too) with nothing applied.
    /// `App::draw_preview` answers what a release would make.
    Draw {
        kind: CreateKind,
        from: Point,
        to: Point,
        #[serde(default)]
        mods: Modifiers,
    },
    /// A handle of a node was dragged by `by` (the total travel since the press, in world units) and let go: the edge or
    /// corner the handle holds moves and the opposite one stays, and a side never gets below 1. The box starts from where the
    /// canvas draws the node. A side that is sized hug or fill and gets another size becomes fixed in the same step, and the
    /// origin of a child of an auto layout frame is not written (the flow places it), as `Action::Resize` does. A drag
    /// that changes nothing makes no step; an unknown node is refused (`not_found`), a value that is not a number
    /// (`invalid`).
    ResizeHandle {
        id: String,
        handle: Handle,
        by: Point,
        #[serde(default)]
        mods: Modifiers,
    },
}
