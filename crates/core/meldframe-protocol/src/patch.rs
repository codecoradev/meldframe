//! Partial change to a node and how it is applied.

use crate::decoration::{Border, Gradient, ImageFill, Shadow};
use crate::enums::{Align, Justify, LayoutMode, ShaderPreset, SizeMode, TextAlign};
use crate::node::Node;
use crate::variables::Bind;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// serde: `Option<Option<T>>` in which an explicit JSON `null` is `Some(None)` (plain serde folds it into `None`).
fn de_double_option<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// Partial change to a node: only the fields present are changed.
#[derive(Debug, Clone, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct NodePatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub w: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub h: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sw: Option<SizeMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sh: Option<SizeMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fill: Option<Bind<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub radius: Option<Bind<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub layout: Option<LayoutMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub gap: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub padding: Option<f64>,
    #[serde(
        default,
        deserialize_with = "de_double_option",
        skip_serializing_if = "Option::is_none",
        rename = "padT"
    )]
    #[ts(optional = nullable, as = "Option<f64>", rename = "padT")]
    /// Padding of the top side (`padT`); the other sides are `padR`, `padB` and `padL`. Three states, as `image`: key
    /// absent = unchanged; `null` = the side is not set (it takes `padding` again); a number = set it.
    pub pad_t: Option<Option<f64>>,
    #[serde(
        default,
        deserialize_with = "de_double_option",
        skip_serializing_if = "Option::is_none",
        rename = "padR"
    )]
    #[ts(optional = nullable, as = "Option<f64>", rename = "padR")]
    pub pad_r: Option<Option<f64>>,
    #[serde(
        default,
        deserialize_with = "de_double_option",
        skip_serializing_if = "Option::is_none",
        rename = "padB"
    )]
    #[ts(optional = nullable, as = "Option<f64>", rename = "padB")]
    pub pad_b: Option<Option<f64>>,
    #[serde(
        default,
        deserialize_with = "de_double_option",
        skip_serializing_if = "Option::is_none",
        rename = "padL"
    )]
    #[ts(optional = nullable, as = "Option<f64>", rename = "padL")]
    pub pad_l: Option<Option<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub align: Option<Align>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub justify: Option<Justify>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub clip: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fill2: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub shader: Option<ShaderPreset>,
    // --- typography and decoration (additive block) ---
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    /// Text node font weight (`fontWeight`): 100..=900 in steps of 100 (400 regular, 700 bold); other values are rejected.
    pub font_weight: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    /// A child of an auto layout frame that leaves the flow and sits at its own `x` and `y` (`absolute`).
    pub absolute: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "marT")]
    #[ts(optional, rename = "marT")]
    /// Margin of the top side (`marT`); the other sides are `marR`, `marB` and `marL`.
    pub mar_t: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "marR")]
    #[ts(optional, rename = "marR")]
    pub mar_r: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "marB")]
    #[ts(optional, rename = "marB")]
    pub mar_b: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "marL")]
    #[ts(optional, rename = "marL")]
    pub mar_l: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    /// Opacity of the node and what is inside it: 0 to 1 (`opacity`).
    pub opacity: Option<Bind<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    /// Text node horizontal alignment in its box (`textAlign`): left, center or right (justify is not supported).
    pub text_align: Option<TextAlign>,
    /// Replaces the whole `border` (width 0 = none; drawn on frame, rect, ellipse and shader nodes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub border: Option<Border>,
    /// Replaces the whole `shadows` list (max 4 drop shadows; `[]` clears).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub shadows: Option<Vec<Shadow>>,
    /// three states. Key absent = unchanged; `null` = remove the image; object = set it.
    #[serde(
        default,
        deserialize_with = "de_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional = nullable, as = "Option<ImageFill>")]
    pub image: Option<Option<ImageFill>>,
    /// Three states, as `image`: key absent = unchanged; `null` = remove the gradient; object = set it.
    #[serde(
        default,
        deserialize_with = "de_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional = nullable, as = "Option<Gradient>")]
    pub gradient: Option<Option<Gradient>>,
}

impl Node {
    pub fn apply_patch(&mut self, p: &NodePatch) {
        if let Some(v) = &p.image {
            self.image = v.clone();
        }
        if let Some(v) = &p.gradient {
            self.gradient = v.clone();
        }
        for (side, v) in [
            (&mut self.pad_t, &p.pad_t),
            (&mut self.pad_r, &p.pad_r),
            (&mut self.pad_b, &p.pad_b),
            (&mut self.pad_l, &p.pad_l),
        ] {
            if let Some(v) = v {
                *side = *v;
            }
        }
        macro_rules! set {
            ($($f:ident),*) => { $( if let Some(v) = &p.$f { self.$f = v.clone(); } )* };
        }
        set!(
            name,
            x,
            y,
            w,
            h,
            sw,
            sh,
            fill,
            fill2,
            radius,
            layout,
            gap,
            padding,
            align,
            justify,
            clip,
            text,
            font_size,
            shader,
            font_weight,
            absolute,
            mar_t,
            mar_r,
            mar_b,
            mar_l,
            opacity,
            text_align,
            border,
            shadows
        );
    }
}
