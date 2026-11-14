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
    /// image over `fill`. Absent = no image (old files stay valid).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub image: Option<ImageFill>,
}
