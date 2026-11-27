//! The camera and the viewport (E8.1.4, part 1): what a pan, a zoom and the size of the canvas come to, as state of the app layer,
//! so that a second front end and a headless test pan, zoom and fit with the same rules. The mapping is
//! `screen = world * zoom + (x, y)`: `x` and `y` are where the world origin sits on the canvas, in CSS pixels from its top left
//! corner, and `zoom` is 1 at 100%. The rules (the limits, the fit, the meaning of a wheel and a pinch) are
//! the rules of this crate; this is the data. A camera change is a view change: it writes nothing to the file and does not
//! touch the undo history.

use crate::input::{Modifiers, Point};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The camera: where the world is on the canvas and how large (`AppState::camera`). Always finite, with a zoom within the limits.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

impl Default for Camera {
    /// The home camera: the world origin at the top left corner of the canvas, at 100%.
    fn default() -> Self {
        Camera {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

/// The size of the canvas in CSS pixels (`AppState::viewport`), as the front end last reported it (`CameraOp::SetViewport`). It
/// is 0 by 0 until it does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Viewport {
    pub w: f64,
    pub h: f64,
}

/// A command of the zoom menu and its keys: one step in or out, 100%, all the content, or the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ZoomCommand {
    In,
    Out,
    Reset,
    Fit,
    FitSelection,
}

/// The unit of the deltas of a wheel event (`WheelEvent.deltaMode` 0, 1 and 2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum WheelUnit {
    #[default]
    Pixel,
    Line,
    Page,
}

/// A wheel event as the platform reports it. The wheel pans (two-finger scroll on a trackpad too); with Ctrl or Command held it
/// zooms, which is also how a browser reports a trackpad pinch. The JSON may leave `unit` and `mods` out (pixels, no key held).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Wheel {
    pub dx: f64,
    pub dy: f64,
    #[serde(default)]
    pub unit: WheelUnit,
    #[serde(default)]
    pub mods: Modifiers,
}

/// What a person does to the camera (`Action::Camera`). Points are screen points, in CSS pixels from the top left corner of the
/// canvas. A number that is not finite, a factor that is not above 0 and a size below 0 are refused (`invalid`) with nothing
/// changed. A zoom is cut to the limits and the camera stays finite; an op that changes nothing raises nothing, and one that
/// changes the camera or the viewport raises `Event::CameraChanged`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CameraOp {
    /// Move the world by `(dx, dy)` screen pixels (a drag with the pan tool or Space held).
    Pan { dx: f64, dy: f64 },
    /// Multiply the zoom by `factor` around the screen point `at`: the world point under it stays under it.
    ZoomAt { factor: f64, at: Point },
    /// A command of the zoom menu (and the zoom keys). In, out and 100% act around the centre of the view; fit shows all the
    /// content and fit selection the selection (all the content when nothing is selected), centred, with room around them.
    Zoom { command: ZoomCommand },
    /// A wheel event at the screen point `at`.
    Wheel { wheel: Wheel, at: Point },
    /// A gesture of two pointers from the pair `from` to the pair `to`: the zoom follows the change of their distance and the
    /// world point under their midpoint ends under the new one, so a pinch also pans.
    Pinch { from: [Point; 2], to: [Point; 2] },
    /// The canvas has this size now.
    SetViewport { w: f64, h: f64 },
}
