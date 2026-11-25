//! The corner radius. The model has one radius for the four corners and the canvas clamps it to half the box, so a
//! `9999px` pill stays a pill. `50%` is the ellipse: a Node of kind ellipse when the box holds nothing, else half of
//! the smaller authored side (a frame has no ellipse form). Corners that differ, an elliptical corner and any other
//! percentage have no field and are refused.
//!
//! The export also writes the radius on the `::after` box of a border, the node's radius clamped to half the box plus
//! how far the box grows (`calc(min(var(--r) * 1px, Wpx, Hpx) + Npx)` when it is bound). When the border is that
//! box, its radius is read as well: it must agree with the element's, and it gives the radius when the element
//! does not say it.

use super::calc::min_plus;
use super::lex::percent;
use super::{Ctx, Paint, Sink};
use crate::css::value::{length, words};
use crate::css::{Props, Style};
use meldframe_protocol::{Bind, BorderPosition};

const CORNERS: [&str; 4] = [
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-right-radius",
    "border-bottom-left-radius",
];
/// The largest radius the model checks.
const MAX_RADIUS: f64 = 100_000.0;
/// How the `::after` box is named in the report.
const AFTER: &str = "::after border-radius";

#[derive(Debug, Clone, Copy, PartialEq)]
enum Corner {
    Px(f64),
    /// `50%`.
    Half,
    /// Any other percentage.
    Pct,
}

type Parsed = Result<Corner, &'static str>;

/// One word of a corner. On the `::after` box (`after`) the `calc(min(...) + N)` form is a length too.
fn word(w: &str, style: &Style, after: bool) -> Parsed {
    if w.ends_with('%') {
        return match percent(w) {
            Some(50.0) => Ok(Corner::Half),
            Some(p) if p >= 0.0 => Ok(Corner::Pct),
            _ => Err("radius_unreadable"),
        };
    }
    let px = match length(w, style.font_size, style.rem) {
        Some(l) => Some(l.px),
        None if after => min_plus(w, style.font_size, style.rem),
        None => None,
    };
    match px {
        Some(px) if px >= 0.0 => Ok(Corner::Px(px)),
        _ => Err("radius_unreadable"),
    }
}

/// One corner: a single value, or two that are the same (a circular corner written the long way).
fn corner(text: &str, style: &Style, after: bool) -> Parsed {
    let w = words(text);
    match w.as_slice() {
        [a] => word(a, style, after),
        [a, b] => {
            let (a, b) = (word(a, style, after)?, word(b, style, after)?);
            if a == b {
                Ok(a)
            } else {
                Err("radius_elliptical")
            }
        }
        _ => Err("radius_unreadable"),
    }
}

/// The four corners of `props`: `None` when none is declared; the one value when they are the same, else
/// `radius_per_corner`. The text is the first corner's, for the report.
fn corners<'a>(props: &'a Props, style: &Style, after: bool) -> Option<(Parsed, &'a str)> {
    if !CORNERS.iter().any(|c| props.contains_key(*c)) {
        return None;
    }
    let text = |c: &str| props.get(c).map_or("0", |p| p.value.trim());
    let all: [Parsed; 4] = CORNERS.map(|c| corner(text(c), style, after));
    let same = all.iter().all(|c| *c == all[0]);
    Some((
        if same {
            all[0]
        } else {
            Err("radius_per_corner")
        },
        text(CORNERS[0]),
    ))
}

/// Put the radius of the element on `out`.
fn resolve(c: Parsed, text: &str, ctx: &Ctx, out: &mut Paint, k: &mut Sink) {
    match c {
        Err("radius_per_corner") => k.refuse(
            "radius_per_corner",
            "border-radius",
            text,
            "corners that differ have no field: the model has one radius",
        ),
        Err(code) => k.refuse(code, "border-radius", text, "not one radius in pixels, rem, em or pt"),
        Ok(Corner::Px(0.0)) => {}
        Ok(Corner::Px(v)) => out.radius = Bind::Lit(capped(v, text, k)),
        Ok(Corner::Half) if !ctx.has_children => out.ellipse = true,
        Ok(Corner::Half) => match (ctx.width, ctx.height) {
            (Some(w), Some(h)) => out.radius = Bind::Lit(capped(w.min(h) / 2.0, text, k)),
            _ => k.refuse(
                "radius_percent_size",
                "border-radius",
                text,
                "50% on a box that holds something is half of its smaller side, which is not authored in pixels",
            ),
        },
        Ok(Corner::Pct) => k.refuse("radius_percent", "border-radius", text, "only 50% is read"),
    }
}

/// Does the radius of the `::after` box (`after`, with the border's growth taken off) say what the element's does.
/// The box carries the radius clamped to half the box: with the size known that is checked, without it a smaller
/// value is accepted, since the clamp cannot be seen.
fn agrees(own: Corner, after: f64, ctx: &Ctx) -> bool {
    let Corner::Px(o) = own else { return false };
    if (after - o).abs() < 1e-6 {
        return true;
    }
    match (ctx.width, ctx.height) {
        (Some(w), Some(h)) => (after - o.min(w / 2.0).min(h / 2.0)).abs() < 1e-6,
        _ => after < o,
    }
}

/// Read the radius of the element, and, when the border is the export's box (`from_box`), the box's.
pub(super) fn read(style: &Style, ctx: &Ctx, from_box: bool, out: &mut Paint, k: &mut Sink) {
    k.steps += 4;
    let own = corners(&style.props, style, false);
    if let Some((c, text)) = own {
        resolve(c, text, ctx, out, k);
    }
    let Some((after, text)) = style
        .after
        .as_ref()
        .filter(|_| from_box)
        .and_then(|a| corners(a, style, true))
    else {
        return;
    };
    k.steps += 4;
    let grow = match out.border.position {
        BorderPosition::Inside => 0.0,
        BorderPosition::Center => out.border.width / 2.0,
        BorderPosition::Outside => out.border.width,
    };
    let differs = |k: &mut Sink| {
        k.refuse(
            "after_radius_differs",
            AFTER,
            text,
            "the box of the border has another radius than the element: the model has one, the element's is kept",
        );
    };
    match (after, own.map(|o| o.0)) {
        (Err(code), _) => k.refuse(code, AFTER, text, "not one radius in pixels, rem, em or pt"),
        (Ok(Corner::Pct), _) => k.refuse("radius_percent", AFTER, text, "only 50% is read"),
        (Ok(Corner::Half), Some(Ok(Corner::Half))) => {}
        (Ok(Corner::Half), Some(_)) => differs(k),
        (Ok(Corner::Half), None) => k.refuse(
            "after_radius_percent",
            AFTER,
            text,
            "50% on the box alone does not say how far the box grows: the radius is not read from it",
        ),
        (Ok(Corner::Px(a)), None) => {
            let r = (a - grow).max(0.0);
            if r > 0.0 {
                out.radius = Bind::Lit(capped(r, text, k));
                k.list(
                    "radius_from_border",
                    AFTER,
                    text,
                    "the element has no radius: it is read from the box of the border, less how far the box grows",
                );
            }
        }
        (Ok(Corner::Px(a)), Some(Ok(o))) => {
            if !agrees(o, (a - grow).max(0.0), ctx) {
                differs(k);
            }
        }
        (Ok(Corner::Px(_)), Some(Err(_))) => differs(k),
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
