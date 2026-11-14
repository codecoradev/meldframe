//! Checks of single values and of whole nodes and patches (no knowledge of the rest of the file).

use crate::{ApplyError, Limits};
use meldframe_order::fracindex;
use meldframe_protocol::{Node, NodePatch};

fn num(field: &'static str, v: f64, lo: f64, hi: f64) -> Result<(), ApplyError> {
    if !v.is_finite() {
        return Err(ApplyError::NotFinite(field));
    }
    if v < lo || v > hi {
        return Err(ApplyError::OutOfRange { field, value: v });
    }
    Ok(())
}

fn strlen(field: &'static str, s: &str, max: usize) -> Result<(), ApplyError> {
    if s.chars().count() > max {
        return Err(ApplyError::TooLong { field, max });
    }
    Ok(())
}

/// A hex colour: `#` followed by 3, 4, 6 or 8 hexadecimal digits.
fn hex_color(field: &'static str, s: &str) -> Result<(), ApplyError> {
    let ok = s.strip_prefix('#').is_some_and(|h| {
        matches!(h.len(), 3 | 4 | 6 | 8) && h.bytes().all(|b| b.is_ascii_hexdigit())
    });
    if ok {
        Ok(())
    } else {
        Err(ApplyError::InvalidColor { field })
    }
}

/// A colour field: bounded length first, then hex form.
fn color(field: &'static str, s: &str) -> Result<(), ApplyError> {
    strlen(field, s, 64)?;
    hex_color(field, s)
}

fn valid_id(id: &str, l: &Limits) -> bool {
    !id.is_empty()
        && id.chars().count() <= l.max_id
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '.'))
}

type Fields<'a> = &'a [(&'static str, Option<f64>)];

/// The numeric fields shared by nodes and patches. `fontSize` must be at least 1, the others at least 0.
fn check_numbers(
    l: &Limits,
    xy: [Option<f64>; 2],
    wh: [Option<f64>; 2],
    others: Fields,
) -> Result<(), ApplyError> {
    for (field, v) in [("x", xy[0]), ("y", xy[1])] {
        if let Some(v) = v {
            num(field, v, -l.max_coord, l.max_coord)?;
        }
    }
    for (field, v) in [("w", wh[0]), ("h", wh[1])] {
        if let Some(v) = v {
            num(field, v, 0.0, l.max_size)?;
        }
    }
    for (field, v) in others {
        if let Some(v) = v {
            let lo = if *field == "fontSize" { 1.0 } else { 0.0 };
            num(field, *v, lo, l.max_size)?;
        }
    }
    Ok(())
}

/// Validate a whole node (as sent by `AddNode`).
pub fn check_node(n: &Node, l: &Limits) -> Result<(), ApplyError> {
    if !valid_id(&n.id, l) {
        return Err(ApplyError::InvalidId(n.id.clone()));
    }
    strlen("name", &n.name, l.max_name)?;
    strlen("text", &n.text, l.max_text)?;
    color("fill", &n.fill)?;
    color("fill2", &n.fill2)?;
    if !n.pos.is_empty() {
        check_pos(&n.pos, l)?;
    }
    check_numbers(
        l,
        [Some(n.x), Some(n.y)],
        [Some(n.w), Some(n.h)],
        &[
            ("radius", Some(n.radius)),
            ("gap", Some(n.gap)),
            ("padding", Some(n.padding)),
            ("fontSize", Some(n.font_size)),
        ],
    )
}

/// Validate a patch: only the fields present are checked.
pub fn check_patch(p: &NodePatch, l: &Limits) -> Result<(), ApplyError> {
    if let Some(s) = &p.name {
        strlen("name", s, l.max_name)?;
    }
    if let Some(s) = &p.text {
        strlen("text", s, l.max_text)?;
    }
    if let Some(s) = &p.fill {
        color("fill", s)?;
    }
    if let Some(s) = &p.fill2 {
        color("fill2", s)?;
    }
    check_numbers(
        l,
        [p.x, p.y],
        [p.w, p.h],
        &[
            ("radius", p.radius),
            ("gap", p.gap),
            ("padding", p.padding),
            ("fontSize", p.font_size),
        ],
    )
}

/// Validate a `pos` from an untrusted source: alphabet, key shape and length.
pub fn check_pos(p: &str, l: &Limits) -> Result<(), ApplyError> {
    if p.len() > l.max_pos {
        return Err(ApplyError::PosTooLong { max: l.max_pos });
    }
    if !fracindex::is_valid(p) {
        return Err(ApplyError::InvalidPos(p.chars().take(32).collect()));
    }
    Ok(())
}
