//! One value of the properties panel as an action carries it: the fields of the inspector that become one `UpdateNode`.

use crate::layout::{Axis, Side};
use crate::state::FillKind;
use meldframe_protocol::{Align, BorderPosition, Justify, LayoutMode, Shadow, SizeMode, TextAlign};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
    /// The weight of a text, 100 to 900 in steps of 100 (the core refuses any other number as `invalid`; a text only).
    FontWeight(u16),
    /// Where the lines of a text sit in its box (a text only).
    TextAlign(TextAlign),
    /// 0 (invisible) to 1; a value outside that range is refused. A bound opacity becomes the number.
    Opacity(f64),
    /// Whether a frame cuts off what is drawn outside it.
    Clip(bool),
    /// Switch the type of the fill, as one undo step. Solid and None take a gradient and an image away; Linear and
    /// Radial make a two-stop gradient from the painted colour when there is none, or change the kind of the one that
    /// is there. A type the node has already is not a change: no step and no refusal. Image is refused as `no_image`
    /// unless the node has one already. A gradient on text is refused as `gradient_on_text`. None replaces the fill with
    /// `none`, so a fill bound to a variable lets the variable go in the same step (undo brings the binding back).
    FillType(FillKind),
    /// The direction of a linear gradient in degrees, kept within one turn. Refused as `no_gradient` without one.
    GradientAngle(f64),
    /// Set the position (0 to 1, held between the neighbouring stops) and the `#hex` colour of one stop.
    GradientStop {
        index: usize,
        pos: f64,
        color: String,
    },
    /// Add a stop in the middle of the widest gap, with the colour of the stop before it. Refused as `limit` at
    /// eight.
    AddGradientStop {},
    /// Remove a stop. Refused as `min_stops` at two.
    RemoveGradientStop(usize),
    /// The width of the border in px (0 is no border). The checks of the range are the core's.
    BorderWidth(f64),
    /// The `#hex` colour of the border.
    BorderColor(String),
    /// Where the border sits against the edge of the box.
    BorderPosition(BorderPosition),
    /// Replace one shadow of the list, as one undo step. Refused as `not_found` past the end.
    SetShadow {
        index: usize,
        shadow: Shadow,
    },
    /// Add a soft drop shadow at the end of the list (drawn on top). Refused as `limit` at `Properties::max_shadows`.
    AddShadow {},
    /// Remove one shadow. Refused as `not_found` past the end.
    RemoveShadow(usize),
    /// Switch a frame between no layout, a row and a column (a frame only: refused as `invalid` on any other node).
    Layout(LayoutMode),
    /// The gap between the children of an auto layout frame (a frame only: refused as `invalid` on any other node).
    Gap(f64),
    /// One side of the padding; the sides that are not set keep the uniform value (a frame only: refused as `invalid` on any other node).
    Padding {
        side: Side,
        value: f64,
    },
    /// All four sides at once: the uniform value, and the sides forget their own (a frame only: refused as `invalid` on any other node).
    PaddingAll(f64),
    /// One side of the margin.
    Margin {
        side: Side,
        value: f64,
    },
    /// How the width or the height is sized.
    Sizing {
        axis: Axis,
        mode: SizeMode,
    },
    /// Where the children sit across the layout (a frame only: refused as `invalid` on any other node).
    Align(Align),
    /// Where the children sit along the layout (a frame only: refused as `invalid` on any other node).
    Justify(Justify),
    /// Take the node out of the flow of its auto layout parent, or put it back.
    Absolute(bool),
}
