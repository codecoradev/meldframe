//! `linear-gradient()` and `radial-gradient()` as the model's `Gradient`: 2 to 8 stops with positions from 0 to 1 that
//! do not go back, an angle for the linear one. Only the forms the model has a field for are read (section 3.6): an
//! angle in `deg` or a side (a corner when the box has a size), percent stops, and the one radial form the export
//! writes, `circle farthest-corner`. A form that is not one of those is refused with its reason, never turned into
//! the nearest one.

use super::lex::{colour_word, inner, percent, split_commas};
use super::{Ctx, Sink};
use crate::css::Style;
use crate::css::value::words;
use meldframe_css::number::{number, starts_like_number};
use meldframe_protocol::{Gradient, GradientKind, GradientStop};

/// The most stops the model holds.
const MAX_STOPS: usize = 8;
/// The most comma-separated parts of one gradient that are looked at.
const MAX_PARTS: usize = 64;
/// The widest angle the model checks (`E_INVALID_GRADIENT`).
const MAX_ANGLE: f64 = 3600.0;

/// Why a gradient is not read: a stable code and what is wrong, for the report.
pub(super) type Why = (&'static str, &'static str);

/// The gradient in `layer`, a whole `linear-gradient(...)` or `radial-gradient(...)` call. `prop` is where it was
/// written, for the report.
pub(super) fn parse(
    name: &str,
    layer: &str,
    style: &Style,
    ctx: &Ctx,
    k: &mut Sink,
) -> Result<Gradient, Why> {
    let body = inner(layer, name).ok_or(("gradient_syntax", "not one call"))?;
    let (parts, cut) = split_commas(body, MAX_PARTS);
    if cut {
        k.cut(
            "gradient_parts_cut",
            "background-image",
            layer,
            "more than 64 parts: the rest are not read",
        );
    }
    k.steps += 1 + body.len() / 64;
    if parts.is_empty() {
        return Err(("gradient_syntax", "no stops"));
    }
    let linear = name.eq_ignore_ascii_case("linear-gradient");
    let (mut angle, mut rest) = (180.0, &parts[..]);
    let first_is_stop = words(parts[0])
        .first()
        .is_some_and(|w| colour_word(w, style).is_some());
    if !first_is_stop {
        // Not a colour: the direction of a linear gradient, the shape of a radial one.
        if linear {
            angle = direction(parts[0], ctx)?;
        } else {
            shape(parts[0])?;
        }
        rest = &parts[1..];
    } else if !linear {
        return Err((
            "gradient_radial_shape",
            "no `circle`: the default is an ellipse, which the model does not have",
        ));
    }
    let stops = stops(rest, style, layer, k)?;
    Ok(Gradient {
        kind: if linear {
            GradientKind::Linear
        } else {
            GradientKind::Radial
        },
        angle,
        stops,
    })
}

/// The angle of a linear gradient from its first part: `<n>deg` or `to <side>[ <side>]`.
fn direction(part: &str, ctx: &Ctx) -> Result<f64, Why> {
    let w = words(part);
    let lower: Vec<String> = w.iter().map(|s| s.to_ascii_lowercase()).collect();
    let bad = ("gradient_config", "not an angle in deg or a side");
    match lower.as_slice() {
        [one] if starts_like_number(one) => match one.strip_suffix("deg").and_then(number) {
            Some(a) if (-MAX_ANGLE..=MAX_ANGLE).contains(&a) => Ok(a),
            Some(_) => Err(("gradient_angle_range", "an angle past 3600 degrees")),
            None => Err(("gradient_angle_unit", "an angle that is not in deg")),
        },
        [to, side] if to == "to" => match side.as_str() {
            "top" => Ok(0.0),
            "right" => Ok(90.0),
            "bottom" => Ok(180.0),
            "left" => Ok(270.0),
            _ => Err(bad),
        },
        [to, a, b] if to == "to" => {
            let (v, h) = match (a.as_str(), b.as_str()) {
                (v @ ("top" | "bottom"), h @ ("left" | "right"))
                | (h @ ("left" | "right"), v @ ("top" | "bottom")) => (v, h),
                _ => return Err(bad),
            };
            // The corner is the direction that makes the 50% line pass through the other two corners: it depends on
            // the box.
            let (Some(w), Some(h_px)) = (ctx.width, ctx.height) else {
                return Err(("gradient_corner_size", "a corner needs the size of the box"));
            };
            if w <= 0.0 || h_px <= 0.0 {
                return Err(("gradient_corner_size", "a corner needs the size of the box"));
            }
            let t = h_px.atan2(w).to_degrees();
            Ok(match (v, h) {
                ("top", "right") => t,
                ("bottom", "right") => 180.0 - t,
                ("bottom", _) => 180.0 + t,
                _ => 360.0 - t,
            })
        }
        _ => Err(bad),
    }
}

/// The first part of a radial gradient: only `circle`, with `farthest-corner` (the default size), is the form the
/// model has.
fn shape(part: &str) -> Result<(), Why> {
    let lower: Vec<String> = words(part).iter().map(|s| s.to_ascii_lowercase()).collect();
    if lower.iter().any(|w| w == "at") {
        return Err(("gradient_radial_position", "`at <position>` is not carried"));
    }
    if lower.iter().any(|w| w == "ellipse") {
        return Err(("gradient_radial_shape", "an ellipse is not carried"));
    }
    if lower
        .iter()
        .any(|w| w != "circle" && w != "farthest-corner")
    {
        return Err((
            "gradient_radial_size",
            "only the size farthest-corner is carried",
        ));
    }
    let count = |n: &str| lower.iter().filter(|w| *w == n).count();
    match (count("circle"), count("farthest-corner")) {
        (1, 0 | 1) => Ok(()),
        (_, 2..) => Err(("gradient_radial_size", "the size is given twice")),
        _ => Err(("gradient_radial_shape", "no `circle`, or it is given twice")),
    }
}

/// The stops of the parts after the direction: a colour and up to two percent positions each.
fn stops(
    parts: &[&str],
    style: &Style,
    layer: &str,
    k: &mut Sink,
) -> Result<Vec<GradientStop>, Why> {
    let mut items: Vec<(String, Option<f64>)> = Vec::new();
    for part in parts {
        let w = words(part);
        let Some(first) = w.first() else {
            return Err(("gradient_syntax", "an empty stop"));
        };
        let Some((hex, clipped)) = colour_word(first, style) else {
            return Err(if starts_like_number(first) {
                ("gradient_hint", "a colour hint between two stops")
            } else {
                ("colour_unreadable", "a stop colour that is not read")
            });
        };
        if clipped {
            k.list(
                "colour_clipped",
                "background-image",
                layer,
                "a stop colour outside sRGB was clamped",
            );
        }
        if w.len() > 3 {
            return Err(("gradient_syntax", "a stop with more than two positions"));
        }
        let mut positions = Vec::new();
        for p in &w[1..] {
            match percent(p) {
                Some(v) if (0.0..=100.0).contains(&v) => positions.push(v),
                Some(_) => return Err(("gradient_stop_range", "a position outside 0% to 100%")),
                None => return Err(("gradient_stop_unit", "a position that is not a percentage")),
            }
        }
        if positions.is_empty() {
            items.push((hex.clone(), None));
        }
        for p in positions {
            items.push((hex.clone(), Some(p)));
        }
    }
    if items.len() < 2 {
        return Err(("gradient_stops_few", "a gradient needs two stops"));
    }
    let n = items.len();
    let mut pos: Vec<Option<f64>> = items.iter().map(|i| i.1).collect();
    pos[0].get_or_insert(0.0);
    pos[n - 1].get_or_insert(100.0);
    // A position before the largest one so far is raised to it, as CSS does.
    let (mut top, mut raised) = (0.0f64, false);
    for p in pos.iter_mut().flatten() {
        if *p < top {
            *p = top;
            raised = true;
        }
        top = top.max(*p);
    }
    if raised {
        k.list(
            "gradient_stop_clamped",
            "background-image",
            layer,
            "a position that went back was raised to the one before it",
        );
    }
    // The stops with no position are spread evenly between the ones that have one.
    let mut i = 0;
    while i < n {
        if pos[i].is_some() {
            i += 1;
            continue;
        }
        let (a, mut j) = (i - 1, i);
        while pos[j].is_none() {
            j += 1;
        }
        let (pa, pb) = (pos[a].unwrap_or(0.0), pos[j].unwrap_or(100.0));
        for (step, slot) in pos[i..j].iter_mut().enumerate() {
            *slot = Some(pa + (pb - pa) * (step + 1) as f64 / (j - a) as f64);
        }
        i = j;
    }
    let mut out: Vec<GradientStop> = items
        .into_iter()
        .zip(pos)
        .map(|((color, _), p)| GradientStop {
            pos: p.unwrap_or(0.0) / 100.0,
            color,
        })
        .collect();
    if out.len() > MAX_STOPS {
        out.truncate(MAX_STOPS);
        k.list(
            "gradient_stops_cut",
            "background-image",
            layer,
            "the model holds 8 stops: the first 8 are kept",
        );
    }
    Ok(out)
}
