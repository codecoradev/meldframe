//! What a new frame or node starts from: the frame presets and the kinds the node tools make. Moved out of `action.rs`,
//! which `Action` fills up to the file ceiling; `action` re-exports both.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A frame size to start a design from. The sizes are design choices (px; the paper sizes are in points at 72 per inch,
/// as design tools draw them), listed here and nowhere else (`AppState.limits.presets` carries the table to a front end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum FramePreset {
    Desktop,
    DesktopHd,
    Tablet,
    Mobile,
    PhoneSmall,
    A4,
    Letter,
}

impl FramePreset {
    /// Every preset, in the order a menu lists them.
    pub const ALL: [FramePreset; 7] = [
        FramePreset::Desktop,
        FramePreset::DesktopHd,
        FramePreset::Tablet,
        FramePreset::Mobile,
        FramePreset::PhoneSmall,
        FramePreset::A4,
        FramePreset::Letter,
    ];

    /// Width and height: a 1440 x 900 laptop, a 1920 x 1080 monitor, an iPad portrait, a current phone, a small phone,
    /// and the two paper sizes.
    pub fn size(self) -> (f64, f64) {
        match self {
            FramePreset::Desktop => (1440.0, 900.0),
            FramePreset::DesktopHd => (1920.0, 1080.0),
            FramePreset::Tablet => (768.0, 1024.0),
            FramePreset::Mobile => (390.0, 844.0),
            FramePreset::PhoneSmall => (375.0, 667.0),
            FramePreset::A4 => (595.0, 842.0),
            FramePreset::Letter => (612.0, 792.0),
        }
    }

    /// The name the new frame gets.
    pub fn name(self) -> &'static str {
        match self {
            FramePreset::Desktop => "Desktop",
            FramePreset::DesktopHd => "Desktop HD",
            FramePreset::Tablet => "Tablet",
            FramePreset::Mobile => "Mobile",
            FramePreset::PhoneSmall => "Phone small",
            FramePreset::A4 => "A4",
            FramePreset::Letter => "Letter",
        }
    }
}

/// A row of the preset table a front end reads from `AppState.limits.presets`: the preset, its name and its size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresetRow {
    pub id: FramePreset,
    pub name: String,
    pub width: f64,
    pub height: f64,
}

impl FramePreset {
    /// The table of presets, in the order a menu lists them.
    pub fn table() -> Vec<PresetRow> {
        FramePreset::ALL
            .into_iter()
            .map(|id| {
                let (width, height) = id.size();
                PresetRow {
                    id,
                    name: id.name().to_owned(),
                    width,
                    height,
                }
            })
            .collect()
    }
}

/// What the node tools can make.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum CreateKind {
    Frame,
    Rectangle,
    Ellipse,
    Text,
}
