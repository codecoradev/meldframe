//! The two small inputs the camera types are made of, which the rest of the app layer reuses: a point, and the modifier keys held.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A point of the canvas in world space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// The modifier keys held with a key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Modifiers {
    #[serde(default)]
    pub ctrl: bool,
    /// Command on a Mac, the Windows key elsewhere.
    #[serde(default)]
    pub meta: bool,
    /// Option on a Mac.
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
}

impl Modifiers {
    /// Ctrl or Command: the key of shortcuts. Either one counts on every platform, as the editor has always taken it.
    pub fn command(&self) -> bool {
        self.ctrl || self.meta
    }
}
