//! Decoration blocks of a node: border, drop shadow and image fill.

use crate::enums::{BorderPosition, GradientKind, ImageFit};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

fn d_border_color() -> String {
    "#000000".into()
}

/// Uniform border on all four sides. `width` 0 = no border. A patch replaces the whole border; members left out
/// take their defaults. Drawn on frame, rect, ellipse and shader nodes (ignored on text).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Border {
    /// Stroke width in px, 0..=100000. 0 = no border.
    #[serde(default)]
    pub width: f64,
    /// Hex color: #rgb, #rgba, #rrggbb or #rrggbbaa.
    #[serde(default = "d_border_color")]
    pub color: String,
    #[serde(default)]
    pub position: BorderPosition,
}

impl Default for Border {
    fn default() -> Self {
        Border {
            width: 0.0,
            color: d_border_color(),
            position: BorderPosition::Inside,
        }
    }
}

fn d_shadow_color() -> String {
    "#00000040".into()
}

/// One drop shadow, drawn behind the node in the shape of the node. Inner shadows are not supported.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Shadow {
    /// Horizontal offset in px (positive = right), |x| <= 100000.
    #[serde(default)]
    pub x: f64,
    /// Vertical offset in px (positive = down), |y| <= 100000.
    #[serde(default)]
    pub y: f64,
    /// Blur radius in px, 0..=100000. 0 = hard edge.
    #[serde(default)]
    pub blur: f64,
    /// Grows (positive) or shrinks (negative) the shadow shape in px, |spread| <= 100000.
    #[serde(default)]
    pub spread: f64,
    /// Hex color: #rgb, #rgba, #rrggbb or #rrggbbaa (alpha allowed). Default #00000040.
    #[serde(default = "d_shadow_color")]
    pub color: String,
}

/// Image painted over the node's `fill` (frame, rect, ellipse, shader). `asset_id` is the SHA-256 (64 lowercase hex
/// characters) of the uploaded file in the same file (asset upload is a separate interface).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ImageFill {
    pub asset_id: String,
    #[serde(default)]
    pub fit: ImageFit,
}

fn d_angle() -> f64 {
    180.0
}

/// One colour of a gradient at a position along it.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GradientStop {
    /// Where the colour is, from 0 (start) to 1 (end).
    pub pos: f64,
    /// Hex colour: #rgb, #rgba, #rrggbb or #rrggbbaa (alpha allowed).
    pub color: String,
}

/// A gradient painted over the node's `fill` and under its image (frame, rect, ellipse, shader; not text). 2 to 8
/// stops with positions from 0 to 1 that do not go back (equal positions make a hard edge).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Gradient {
    #[serde(default)]
    pub kind: GradientKind,
    /// Direction of a linear gradient in degrees, as CSS `linear-gradient`: 0 points up, 90 right, 180 down (the
    /// default). A radial gradient ignores it.
    #[serde(default = "d_angle")]
    pub angle: f64,
    pub stops: Vec<GradientStop>,
}
