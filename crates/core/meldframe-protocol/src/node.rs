//! The node: one element of a file's tree.

use crate::decoration::{Border, ImageFill, Shadow};
use crate::enums::{Align, Justify, LayoutMode, NodeKind, ShaderPreset, SizeMode, TextAlign};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

fn d_w() -> f64 {
    100.0
}
fn d_gap() -> f64 {
    8.0
}
fn d_pad() -> f64 {
    16.0
}
fn d_true() -> bool {
    true
}
fn d_fill() -> String {
    "#3f3f46".into()
}
fn d_fill2() -> String {
    "#22d3ee".into()
}
fn d_weight() -> u16 {
    400
}
fn d_one() -> f64 {
    1.0
}

/// One node of a file's tree (same shape as the node objects in the web client).
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Node {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub kind: NodeKind,
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Position among siblings (base-62 fractional index, see the document core's fractional index). Effective sibling order is
    /// (pos, id). Empty = not assigned yet (old documents/clients): the core places the node after the last sibling
    /// (`AddNode`) or migrates it by the legacy list order.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    pub pos: String,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default = "d_w")]
    pub w: f64,
    #[serde(default = "d_w")]
    pub h: f64,
    #[serde(default)]
    pub sw: SizeMode,
    #[serde(default)]
    pub sh: SizeMode,
    #[serde(default = "d_fill")]
    pub fill: String,
    #[serde(default = "d_fill2")]
    pub fill2: String,
    #[serde(default)]
    pub radius: f64,
    #[serde(default)]
    pub layout: LayoutMode,
    #[serde(default = "d_gap")]
    pub gap: f64,
    #[serde(default = "d_pad")]
    pub padding: f64,
    /// Padding per side (`padT`, `padR`, `padB`, `padL`). A side that is not set takes `padding`, so a file from before
    /// the sides existed lays out as before; read them through [`Node::pad`].
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "padT")]
    #[ts(optional, rename = "padT")]
    pub pad_t: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "padR")]
    #[ts(optional, rename = "padR")]
    pub pad_r: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "padB")]
    #[ts(optional, rename = "padB")]
    pub pad_b: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "padL")]
    #[ts(optional, rename = "padL")]
    pub pad_l: Option<f64>,
    #[serde(default)]
    pub align: Align,
    #[serde(default)]
    pub justify: Justify,
    #[serde(default = "d_true")]
    pub clip: bool,
    #[serde(default)]
    pub text: String,
    #[serde(default = "d_pad")]
    pub font_size: f64,
    #[serde(default)]
    pub shader: ShaderPreset,
    // --- typography and decoration (additive block) ---
    /// Font weight of text: 100..=900 in steps of 100 (400 = regular, 700 = bold). Only meaningful on text nodes.
    #[serde(default = "d_weight")]
    pub font_weight: u16,
    /// Horizontal alignment of the text within the node box (left, center, right). Only meaningful on text nodes.
    #[serde(default)]
    pub text_align: TextAlign,
    /// Uniform border (see [`Border`]); width 0 = none.
    #[serde(default)]
    pub border: Border,
    /// Drop shadows, at most 4, drawn in list order (first = bottom). Empty = none. A patch replaces the whole list.
    #[serde(default)]
    pub shadows: Vec<Shadow>,
    /// Margin per side, in world units (`marT`, `marR`, `marB`, `marL`; default 0): room around the node inside an auto
    /// layout frame. A frame without layout ignores it. Read it through [`Node::margin`].
    #[serde(default, rename = "marT")]
    #[ts(rename = "marT")]
    pub mar_t: f64,
    #[serde(default, rename = "marR")]
    #[ts(rename = "marR")]
    pub mar_r: f64,
    #[serde(default, rename = "marB")]
    #[ts(rename = "marB")]
    pub mar_b: f64,
    #[serde(default, rename = "marL")]
    #[ts(rename = "marL")]
    pub mar_l: f64,
    /// Opacity of the node and what is inside it: 0 (invisible) to 1 (opaque, the default; old files have none).
    #[serde(default = "d_one")]
    pub opacity: f64,
    /// image over `fill`. Absent = no image (old files stay valid).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub image: Option<ImageFill>,
}

/// The padding of a node on each side, in world units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pad {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Pad {
    /// Left plus right.
    pub fn horizontal(&self) -> f64 {
        self.left + self.right
    }

    /// Top plus bottom.
    pub fn vertical(&self) -> f64 {
        self.top + self.bottom
    }
}

impl Node {
    /// The margin on each side.
    pub fn margin(&self) -> Pad {
        Pad {
            top: self.mar_t,
            right: self.mar_r,
            bottom: self.mar_b,
            left: self.mar_l,
        }
    }

    /// The padding on each side: the side's own value, or `padding` when it is not set.
    pub fn pad(&self) -> Pad {
        Pad {
            top: self.pad_t.unwrap_or(self.padding),
            right: self.pad_r.unwrap_or(self.padding),
            bottom: self.pad_b.unwrap_or(self.padding),
            left: self.pad_l.unwrap_or(self.padding),
        }
    }
}
