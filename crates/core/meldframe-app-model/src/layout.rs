//! The layout fields of the properties panel: the sides of padding and margin, the axes of sizing, and the notes that
//! say a setting has no effect where the node is.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A value for each side of a box, in px.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sides {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

/// A side of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

/// An arrow key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// An axis of the size of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Axis {
    Width,
    Height,
}

/// A setting of the selected node that has no effect where it is. Each one is a rule of `docs/fidelity-contract.md`
/// (G1 and the table of section 6); the front end words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "note", rename_all = "camelCase", deny_unknown_fields)]
pub enum LayoutNote {
    /// `fill` on an axis where the parent has no auto layout or hugs: the export keeps the node's own size (G1).
    FillUnderHug { axis: Axis },
    /// `fill` on an absolute node: it is out of the flow, so it keeps its own size on that axis, whatever the parent is.
    AbsoluteFillIgnored { axis: Axis },
    /// `hug` on a rectangle, an ellipse, a shader or a frame without auto layout: laid out as `fixed`.
    HugIgnored { axis: Axis },
    /// A margin on a node that is not in the flow of an auto layout frame (no such parent, or the node is absolute): it
    /// takes no room there, because the node sits where its own position says.
    MarginIgnored {},
    /// `absolute` on a node whose parent has no auto layout: every child of such a frame sits at its own position.
    AbsoluteIgnored {},
}
