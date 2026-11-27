//! The two overlays of the layout (audit finding T11, issue 324): the shortcut sheet and the command palette. Whether one
//! is open is state of the app layer, so that a second front end (a headless one among them) opens and closes them with
//! the same rules: one is open at a time, and the palette stays shut while a dialog is open. Menus, tooltips and popovers are
//! not here: their open state is focus and DOM behaviour, and stays with the component library.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// An overlay of the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Overlay {
    /// The sheet of every key and gesture the editor answers to.
    Help,
    /// The command palette.
    Palette,
}

/// What a person does with an overlay (`Action::Overlay`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum OverlayOp {
    /// Open the overlay; the one that was open closes. The palette is not opened while a dialog is open (an Export, Export
    /// variables, Import variables or manage dialog, or the bind picker): nothing happens and nothing is said, as two
    /// overlays would stack and the first would stay up behind the palette.
    Open { overlay: Overlay },
    /// Close the overlay when it is the one that is open. A close of the other one changes nothing.
    Close { overlay: Overlay },
    /// Close the overlay when it is open, else open it as `Open` does.
    Toggle { overlay: Overlay },
}

impl OverlayOp {
    /// The overlay the op is about.
    pub fn overlay(&self) -> Overlay {
        match self {
            OverlayOp::Open { overlay }
            | OverlayOp::Close { overlay }
            | OverlayOp::Toggle { overlay } => *overlay,
        }
    }
}
