//! What typed text comes to (`TypedText::Property` of `Action::Typed`): the parse of a field of the properties panel, the range of the
//! field, and the decision that there is nothing to do. One place for the three variants of `num` the front end had.
//! A text that is empty or says what the field already says is `Typed::Nothing` (no action, no undo step, no refusal);
//! a number outside the range of the field is refused as `invalid_value`. A colour is not checked here: whether it is
//! one is the core's to say, so every colour field refuses a bad text with the same code.

use crate::property::within_turn;
use meldframe_app_model::{Properties, Property, ShadowPart, TextField};

/// The smallest width or height a person can type, in px.
pub const MIN_SIZE: f64 = 1.0;

/// What one typed text makes of a node.
#[derive(Debug, Clone, PartialEq)]
pub enum Typed {
    /// Nothing to send and nothing to say.
    Nothing,
    /// A value of the properties panel (one `UpdateNode`).
    Property(Property),
    /// The node moves to `(x, y)`.
    Move { x: f64, y: f64 },
    /// The node gets this size, keeping its origin.
    Resize { w: f64, h: f64 },
}

const INVALID: &str = "invalid_value";
/// A shadow or a gradient stop that is not in the node any more (the node is: `not_found` is the node itself).
const PART_GONE: &str = "part_not_found";

/// A finite number, or the refusal. `trimmed` is not empty.
fn number(trimmed: &str) -> Result<f64, &'static str> {
    match trimmed.parse::<f64>() {
        // `+ 0.0` turns a typed `-0` into `0.0`: a stored coordinate is never `-0.0`.
        Ok(v) if v.is_finite() => Ok(v + 0.0),
        _ => Err(INVALID),
    }
}

/// A number that must be `min` or more (`strict`: above it).
fn at_least(trimmed: &str, min: f64, strict: bool) -> Result<f64, &'static str> {
    let v = number(trimmed)?;
    if v < min || (strict && v == min) {
        return Err(INVALID);
    }
    Ok(v)
}

/// A percentage from 0 to 100 as the file holds it, 0 to 1, with one decimal of the percentage kept.
fn percent(trimmed: &str) -> Result<f64, &'static str> {
    let v = number(trimmed)?;
    if !(0.0..=100.0).contains(&v) {
        return Err(INVALID);
    }
    Ok((v * 10.0).round() / 1000.0)
}

fn changed<T: PartialEq>(now: T, next: T, make: impl FnOnce(T) -> Typed) -> Typed {
    if now == next {
        Typed::Nothing
    } else {
        make(next)
    }
}

/// What `raw`, typed in `field` of the node whose panel is `p`, comes to; or the code of the refusal.
pub fn typed(p: &Properties, field: &TextField, raw: &str) -> Result<Typed, &'static str> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(Typed::Nothing);
    }
    let prop = |v| Typed::Property(v);
    Ok(match field {
        TextField::Name {} => changed(p.name.as_str(), t, |v| prop(Property::Name(v.into()))),
        TextField::Fill {} => text_colour(t, &p.fill, |v| prop(Property::Fill(v))),
        TextField::Radius {} => changed(p.radius, at_least(t, 0.0, false)?, |v| {
            prop(Property::Radius(v))
        }),
        TextField::FontSize {} => changed(p.font_size, at_least(t, 0.0, true)?, |v| {
            prop(Property::FontSize(v))
        }),
        TextField::Opacity {} => changed(p.opacity, percent(t)?, |v| prop(Property::Opacity(v))),
        TextField::X {} => {
            let v = number(t)?;
            if v == p.x || p.position_locked {
                Typed::Nothing
            } else {
                Typed::Move { x: v, y: p.y }
            }
        }
        TextField::Y {} => {
            let v = number(t)?;
            if v == p.y || p.position_locked {
                Typed::Nothing
            } else {
                Typed::Move { x: p.x, y: v }
            }
        }
        TextField::W {} => {
            let v = at_least(t, MIN_SIZE, false)?;
            changed(p.w, v, |w| Typed::Resize { w, h: p.h })
        }
        TextField::H {} => {
            let v = at_least(t, MIN_SIZE, false)?;
            changed(p.h, v, |h| Typed::Resize { w: p.w, h })
        }
        TextField::BorderWidth {} => changed(p.border.width, at_least(t, 0.0, false)?, |v| {
            prop(Property::BorderWidth(v))
        }),
        TextField::BorderColor {} => {
            text_colour(t, &p.border.color, |v| prop(Property::BorderColor(v)))
        }
        TextField::Shadow { index, part } => {
            let Some(current) = p.shadows.get(*index) else {
                return Err(PART_GONE);
            };
            let mut next = current.clone();
            match part {
                ShadowPart::Color {} => {
                    if t.eq_ignore_ascii_case(&current.color) {
                        return Ok(Typed::Nothing);
                    }
                    next.color = t.to_string();
                }
                ShadowPart::X {} => next.x = number(t)?,
                ShadowPart::Y {} => next.y = number(t)?,
                ShadowPart::Spread {} => next.spread = number(t)?,
                ShadowPart::Blur {} => next.blur = at_least(t, 0.0, false)?,
            }
            changed(current, &next, |_| {
                prop(Property::SetShadow {
                    index: *index,
                    shadow: next.clone(),
                })
            })
        }
        TextField::GradientAngle {} => {
            let Some(g) = &p.gradient else {
                return Err("no_gradient");
            };
            // The stored angle is kept within one turn, so 360 typed over 0 is no change.
            let v = number(t)?;
            changed(g.angle, within_turn(v), |_| {
                prop(Property::GradientAngle(v))
            })
        }
        TextField::StopPos { index } => {
            let Some(stop) = p.gradient.as_ref().and_then(|g| g.stops.get(*index)) else {
                return Err(PART_GONE);
            };
            changed(stop.pos, percent(t)?, |pos| {
                prop(Property::GradientStop {
                    index: *index,
                    pos,
                    color: stop.color.clone(),
                })
            })
        }
        TextField::StopColor { index } => {
            let Some(stop) = p.gradient.as_ref().and_then(|g| g.stops.get(*index)) else {
                return Err(PART_GONE);
            };
            text_colour(t, &stop.color, |color| {
                prop(Property::GradientStop {
                    index: *index,
                    pos: stop.pos,
                    color,
                })
            })
        }
        TextField::Gap {} => changed(p.gap, at_least(t, 0.0, false)?, |v| prop(Property::Gap(v))),
        TextField::PaddingAll {} => {
            let v = at_least(t, 0.0, false)?;
            changed(p.padding_uniform, Some(v), |_| {
                prop(Property::PaddingAll(v))
            })
        }
        TextField::Padding { side } => {
            let v = at_least(t, 0.0, false)?;
            changed(side_of(&p.padding, *side), v, |value| {
                prop(Property::Padding { side: *side, value })
            })
        }
        TextField::Margin { side } => {
            let v = at_least(t, 0.0, false)?;
            changed(side_of(&p.margin, *side), v, |value| {
                prop(Property::Margin { side: *side, value })
            })
        }
    })
}

fn side_of(s: &meldframe_app_model::Sides, side: meldframe_app_model::Side) -> f64 {
    use meldframe_app_model::Side;
    match side {
        Side::Top => s.top,
        Side::Right => s.right,
        Side::Bottom => s.bottom,
        Side::Left => s.left,
    }
}

/// A colour text: nothing for the colour the field has (any case), else it goes to the core as typed.
fn text_colour(trimmed: &str, now: &str, make: impl FnOnce(String) -> Typed) -> Typed {
    if trimmed.eq_ignore_ascii_case(now) {
        Typed::Nothing
    } else {
        make(trimmed.to_string())
    }
}
