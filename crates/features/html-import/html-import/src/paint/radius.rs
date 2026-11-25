//! The corner radius. The model has one radius for the four corners and the canvas clamps it to half the box, so a
//! `9999px` pill stays a pill. `50%` is the ellipse: a Node of kind ellipse when the box holds nothing, else half of
//! the smaller authored side (a frame has no ellipse form). Corners that differ, an elliptical corner and any other
//! percentage have no field and are refused.

use super::lex::percent;
use super::{Ctx, Paint, Sink};
use crate::css::Style;
use crate::css::value::{length, words};
use meldframe_protocol::Bind;

const CORNERS: [&str; 4] = [
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-right-radius",
    "border-bottom-left-radius",
];
/// The largest radius the model checks.
const MAX_RADIUS: f64 = 100_000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Corner {
    Px(f64),
    /// `50%`.
    Half,
    /// Any other percentage.
    Pct,
}

type Parsed = Result<Corner, &'static str>;

fn word(w: &str, style: &Style) -> Parsed {
    if w.ends_with('%') {
        return match percent(w) {
            Some(50.0) => Ok(Corner::Half),
            Some(p) if p >= 0.0 => Ok(Corner::Pct),
            _ => Err("radius_unreadable"),
        };
    }
    match length(w, style.font_size, style.rem) {
        Some(l) if l.px >= 0.0 => Ok(Corner::Px(l.px)),
        _ => Err("radius_unreadable"),
    }
}

/// One corner: a single value, or two that are the same (a circular corner written the long way).
fn corner(text: &str, style: &Style) -> Parsed {
    let w = words(text);
    match w.as_slice() {
        [a] => word(a, style),
        [a, b] => {
            let (a, b) = (word(a, style)?, word(b, style)?);
            if a == b {
                Ok(a)
            } else {
                Err("radius_elliptical")
            }
        }
        _ => Err("radius_unreadable"),
    }
}

/// The radius of the four corners in `style`, as the number of its element.
pub(super) fn read(style: &Style, ctx: &Ctx, out: &mut Paint, k: &mut Sink) {
    if !CORNERS.iter().any(|c| style.get(c).is_some()) {
        return;
    }
    k.steps += 4;
    let text = |c: &str| style.get(c).unwrap_or("0").trim();
    let corners: [Parsed; 4] = CORNERS.map(|c| corner(text(c), style));
    let value = text(CORNERS[0]);
    if corners.iter().any(|c| *c != corners[0]) {
        k.refuse(
            "radius_per_corner",
            "border-radius",
            value,
            "corners that differ have no field: the model has one radius",
        );
        return;
    }
    match corners[0] {
        Err(code) => k.refuse(code, "border-radius", value, "not one radius in pixels, rem, em or pt"),
        Ok(Corner::Px(0.0)) => {}
        Ok(Corner::Px(v)) => out.radius = Bind::Lit(capped(v, value, k)),
        Ok(Corner::Half) if !ctx.has_children => out.ellipse = true,
        Ok(Corner::Half) => match (ctx.width, ctx.height) {
            (Some(w), Some(h)) => out.radius = Bind::Lit(capped(w.min(h) / 2.0, value, k)),
            _ => k.refuse(
                "radius_percent_size",
                "border-radius",
                value,
                "50% on a box that holds something is half of its smaller side, which is not authored in pixels",
            ),
        },
        Ok(Corner::Pct) => k.refuse(
            "radius_percent",
            "border-radius",
            value,
            "only 50% is read",
        ),
    }
}

fn capped(v: f64, text: &str, k: &mut Sink) -> f64 {
    if v > MAX_RADIUS {
        k.list(
            "radius_capped",
            "border-radius",
            text,
            "the model holds at most 100000",
        );
        return MAX_RADIUS;
    }
    v
}
