//! How far a box paints beyond its layout box: the reach of its border and its shadows (finding T16 of the thin UI audit,
//! decision 43). It is worked out here once, as the room on each side of the box, and the canvas's capture box reads it
//! from the scene instead of working it out again (`captureRectOf`, `captureBounds` in `packages/web-canvas`).
//!
//! What counts, as the canvas draws it and the export writes it (contract, section 4.2):
//!
//! - a border takes no room: an outside one lies wholly outside the box, a centred one half of its width, an inside one
//!   none;
//! - a shadow that paints (its alpha is above 0) covers the box grown by its spread on every side and moved by its offset,
//!   and its blur reaches past that by the blur's radius. A spread that leaves nothing of the box (it shrinks a side to 0
//!   or less) leaves nothing of the shadow. A shadow's own corner radius does not change what it covers;
//! - the offset, the spread and the blur each count for at most [`MAX_REACH`], so a far or a very blurred shadow does not
//!   make a huge capture (only the reach is capped, not the drawing).
//!
//! The box is always inside its own bounds, so no side is below 0. A box with nothing that reaches out has
//! [`PaintBounds::is_flush`] true, and that is what its JSON leaves out.

use crate::{PaintedBorder, PaintedBox, PaintedShadow};
use meldframe_protocol::BorderPosition;
use serde::Serialize;

/// The most that an offset, a spread or a blur counts for in the reach, in world units. It is `MAX_BLUR` of
/// `packages/web-paint/src/shadow.ts`, the cap the canvas puts on the blur it draws, so a capture makes room for as much
/// blur as the canvas draws; `packages/web-link/src/scene.test.ts` reads the shared fixtures against that constant.
pub const MAX_REACH: f64 = 1024.0;

/// The room the paint of a box takes beyond its layout box, on each side, in world units: none below 0. Added to the box's
/// own rectangle it is the rectangle a capture of the box covers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct PaintBounds {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl PaintBounds {
    /// True when the box paints nowhere but inside its own rectangle.
    pub fn is_flush(&self) -> bool {
        *self == PaintBounds::default()
    }

    /// The reach of a `w` by `h` box with this border and these shadows (those of [`PaintedBox`]: the finite, hex ones, a
    /// blur not below 0).
    pub fn of(w: f64, h: f64, border: Option<&PaintedBorder>, shadows: &[PaintedShadow]) -> Self {
        let band = border.map_or(0.0, |b| match b.position {
            BorderPosition::Outside => b.width,
            BorderPosition::Center => b.width / 2.0,
            BorderPosition::Inside => 0.0,
        });
        let mut out = PaintBounds {
            left: band,
            top: band,
            right: band,
            bottom: band,
        };
        for s in shadows.iter().filter(|s| s.alpha > 0.0) {
            let spread = capped(s.spread);
            let (sw, sh) = (w + 2.0 * spread, h + 2.0 * spread);
            if !(sw > 0.0 && sh > 0.0) {
                continue;
            }
            let blur = s.blur.clamp(0.0, MAX_REACH);
            // The shadow's rectangle in the box's own terms, with its blur.
            let (x0, y0) = (capped(s.x) - spread - blur, capped(s.y) - spread - blur);
            let (x1, y1) = (x0 + sw + 2.0 * blur, y0 + sh + 2.0 * blur);
            out.left = further(out.left, -x0);
            out.top = further(out.top, -y0);
            out.right = further(out.right, x1 - w);
            out.bottom = further(out.bottom, y1 - h);
        }
        out
    }
}

/// The larger of two sides, where a tie keeps `have`, so that a side that stays 0 never turns into -0 (which JSON writes
/// as `-0.0`).
fn further(have: f64, other: f64) -> f64 {
    if other > have { other } else { have }
}

fn capped(v: f64) -> f64 {
    v.clamp(-MAX_REACH, MAX_REACH)
}

impl PaintedBox {
    /// Works `paint_bounds` out again from the size, the border and the shadows the box has now. `resolve_laid_out` calls it
    /// after the layout has given the box its size: a shadow's reach depends on it.
    pub(crate) fn fit_paint_bounds(&mut self) {
        self.paint_bounds = PaintBounds::of(self.w, self.h, self.border.as_ref(), &self.shadows);
    }
}
