//! The result of layout: a box in file coordinates.

use serde::{Deserialize, Serialize};

/// A node's box in the coordinates of its frame's root, in fractional pixels (no rounding; see
/// rule G5 in `docs/fidelity-contract.md`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
