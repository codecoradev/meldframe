//! The fields of the properties panel that take typed text (`TypedText::Property` of `Action::Typed`). A front end names the
//! field and sends what the person typed; the parse, the range and the decision that nothing changes are the app layer's
//! (`meldframe-app`, `parts::typed`).

use crate::layout::Side;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A field of the selected node that holds typed text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TextField {
    /// The name of the node: any text but an empty one.
    Name {},
    /// A `#hex` colour or the word `none`. Whether it is a colour is the core's to say (`E_INVALID_COLOR`).
    Fill {},
    /// The corner radius in px: 0 or more.
    Radius {},
    /// The font size in px: above 0.
    FontSize {},
    /// Typed as a percentage from 0 to 100 (one decimal); the file holds 0 to 1.
    Opacity {},
    /// The position moves the node and keeps the other coordinate; nothing while the layout decides it
    /// (`Properties::position_locked`).
    X {},
    Y {},
    /// The size resizes the node and keeps its origin and the other side: 1 px or more.
    W {},
    H {},
    /// The width of the border in px: 0 (no border) or more.
    BorderWidth {},
    /// A `#hex` colour; the core says whether it is one.
    BorderColor {},
    /// One part of one shadow of the list.
    Shadow {
        index: usize,
        part: ShadowPart,
    },
    /// The direction of a linear gradient in degrees (kept within one turn).
    GradientAngle {},
    /// The position of a stop of the gradient as a percentage from 0 to 100 (rounded to a tenth); the file holds 0 to 1.
    StopPos {
        index: usize,
    },
    /// The `#hex` colour of a stop of the gradient.
    StopColor {
        index: usize,
    },
    /// The gap of an auto layout frame in px: 0 or more.
    Gap {},
    /// All four sides of the padding at once.
    PaddingAll {},
    Padding {
        side: Side,
    },
    Margin {
        side: Side,
    },
}

/// A part of a shadow. The numbers are in px (the blur is 0 or more); the colour is a `#hex` colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ShadowPart {
    X {},
    Y {},
    Blur {},
    Spread {},
    Color {},
}

/// A text a person typed, with where it was typed (`Action::Typed`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TypedText {
    /// A field of the properties panel of the selected node `id`. `not_found` for an unknown node, `not_selected` for
    /// another one than the selected.
    Property {
        id: String,
        field: TextField,
        text: String,
    },
    /// The value field of one mode of a variable: a literal as `SetVariableValue` reads it, and nothing for empty text.
    VariableValue {
        variable: String,
        mode: String,
        text: String,
    },
}
