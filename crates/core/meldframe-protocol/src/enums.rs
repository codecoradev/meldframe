//! Closed sets of values used by nodes. Serialized as lowercase strings.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum NodeKind {
    #[default]
    Frame,
    Rect,
    Ellipse,
    Text,
    Shader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum SizeMode {
    #[default]
    Fixed,
    Hug,
    Fill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum LayoutMode {
    #[default]
    None,
    Row,
    Column,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    Between,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ShaderPreset {
    #[default]
    Aurora,
    Waves,
    Grain,
}

/// Horizontal alignment of text inside its box (`justify` is not supported).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Where a border is drawn relative to the node box: `inside` stays within the box, `outside` grows beyond it,
/// `center` straddles the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum BorderPosition {
    #[default]
    Inside,
    Center,
    Outside,
}

/// How an image fills its node's box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ImageFit {
    /// Scale to cover the whole box, cropping the overflow (default).
    #[default]
    Cover,
    /// Scale to fit entirely inside the box, keeping the aspect ratio; the node `fill` shows around it.
    Contain,
    /// Stretch to the box, ignoring the aspect ratio.
    Fill,
}

/// The shape of a gradient.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum GradientKind {
    /// Colours change along a line at `angle` (default).
    #[default]
    Linear,
    /// Colours change from the centre of the box outwards, as a circle that reaches the farthest corner.
    Radial,
}
