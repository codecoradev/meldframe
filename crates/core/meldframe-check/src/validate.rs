//! Checks of single values and of whole nodes and patches (no knowledge of the rest of the file).

use crate::{ApplyError, Limits};
use meldframe_order::fracindex;
use meldframe_protocol::{Bind, Border, Gradient, ImageFill, Node, NodeKind, NodePatch, Shadow};

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

/// The fill: a colour, the word `none` for no fill, or a binding (the variable is checked against the file).
fn fill(b: &Bind<String>) -> Result<(), ApplyError> {
    match b {
        Bind::Var(_) => Ok(()),
        Bind::Lit(s) if s == "none" => Ok(()),
        Bind::Lit(s) => color("fill", s),
    }
}

/// A number field that may be bound: the literal is checked, a binding is checked against the file.
fn bound_num(field: &'static str, b: &Bind<f64>, lo: f64, hi: f64) -> Result<(), ApplyError> {
    match b {
        Bind::Lit(v) => num(field, *v, lo, hi),
        Bind::Var(_) => Ok(()),
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

/// The canonical form of an asset id (lowercase hex SHA-256). Whether the asset exists is checked by the
/// server, not here.
fn check_image(im: &ImageFill) -> Result<(), ApplyError> {
    let canonical = im.asset_id.len() == 64
        && im
            .asset_id
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if canonical {
        Ok(())
    } else {
        Err(ApplyError::InvalidAssetId)
    }
}

fn font_weight(w: u16) -> Result<(), ApplyError> {
    if (100..=900).contains(&w) && w.is_multiple_of(100) {
        Ok(())
    } else {
        Err(ApplyError::InvalidFontWeight(w))
    }
}

/// A gradient: 2 to `max_gradient_stops` stops with positions from 0 to 1 that do not go back, hex colours, a finite
/// angle within a few turns.
fn check_gradient(g: &Gradient, l: &Limits) -> Result<(), ApplyError> {
    num("gradient.angle", g.angle, -3600.0, 3600.0)?;
    if g.stops.len() > l.max_gradient_stops {
        return Err(ApplyError::TooMany {
            field: "gradient.stops",
            max: l.max_gradient_stops,
        });
    }
    if g.stops.len() < 2 {
        return Err(ApplyError::InvalidGradient(
            "gradient.stops needs at least 2 stops",
        ));
    }
    let mut last = 0.0;
    for s in &g.stops {
        num("gradient.stops.pos", s.pos, 0.0, 1.0)?;
        if s.pos < last {
            return Err(ApplyError::InvalidGradient(
                "gradient.stops positions must be in order (not going back)",
            ));
        }
        last = s.pos;
        color("gradient.stops.color", &s.color)?;
    }
    Ok(())
}

fn check_border(b: &Border, l: &Limits) -> Result<(), ApplyError> {
    num("border.width", b.width, 0.0, l.max_size)?;
    color("border.color", &b.color)
}

fn check_shadows(v: &[Shadow], l: &Limits) -> Result<(), ApplyError> {
    if v.len() > l.max_shadows {
        return Err(ApplyError::TooMany {
            field: "shadows",
            max: l.max_shadows,
        });
    }
    for s in v {
        num("shadows.x", s.x, -l.max_size, l.max_size)?;
        num("shadows.y", s.y, -l.max_size, l.max_size)?;
        num("shadows.blur", s.blur, 0.0, l.max_size)?;
        num("shadows.spread", s.spread, -l.max_size, l.max_size)?;
        color("shadows.color", &s.color)?;
    }
    Ok(())
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
    if let Some(g) = &n.gradient {
        check_gradient(g, l)?;
        if n.kind == NodeKind::Text {
            return Err(ApplyError::GradientOnText);
        }
    }
    if let Some(im) = &n.image {
        check_image(im)?;
        if n.kind == NodeKind::Text {
            return Err(ApplyError::ImageOnText);
        }
    }
    if !valid_id(&n.id, l) {
        return Err(ApplyError::InvalidId(n.id.clone()));
    }
    strlen("name", &n.name, l.max_name)?;
    strlen("text", &n.text, l.max_text)?;
    fill(&n.fill)?;
    color("fill2", &n.fill2)?;
    if !n.pos.is_empty() {
        check_pos(&n.pos, l)?;
    }
    font_weight(n.font_weight)?;
    bound_num("opacity", &n.opacity, 0.0, 1.0)?;
    check_border(&n.border, l)?;
    check_shadows(&n.shadows, l)?;
    check_numbers(
        l,
        [Some(n.x), Some(n.y)],
        [Some(n.w), Some(n.h)],
        &[
            ("radius", n.radius.lit().copied()),
            ("gap", Some(n.gap)),
            ("padding", Some(n.padding)),
            ("marT", Some(n.mar_t)),
            ("marR", Some(n.mar_r)),
            ("marB", Some(n.mar_b)),
            ("marL", Some(n.mar_l)),
            ("padT", n.pad_t),
            ("padR", n.pad_r),
            ("padB", n.pad_b),
            ("padL", n.pad_l),
            ("fontSize", Some(n.font_size)),
        ],
    )
}

/// Validate a patch: only the fields present are checked.
pub fn check_patch(p: &NodePatch, l: &Limits) -> Result<(), ApplyError> {
    if let Some(Some(im)) = &p.image {
        check_image(im)?;
    }
    if let Some(Some(g)) = &p.gradient {
        check_gradient(g, l)?;
    }
    if let Some(s) = &p.name {
        strlen("name", s, l.max_name)?;
    }
    if let Some(s) = &p.text {
        strlen("text", s, l.max_text)?;
    }
    if let Some(s) = &p.fill {
        fill(s)?;
    }
    if let Some(s) = &p.fill2 {
        color("fill2", s)?;
    }
    if let Some(w) = p.font_weight {
        font_weight(w)?;
    }
    if let Some(o) = &p.opacity {
        bound_num("opacity", o, 0.0, 1.0)?;
    }
    if let Some(b) = &p.border {
        check_border(b, l)?;
    }
    if let Some(s) = &p.shadows {
        check_shadows(s, l)?;
    }
    check_numbers(
        l,
        [p.x, p.y],
        [p.w, p.h],
        &[
            ("radius", p.radius.as_ref().and_then(Bind::lit).copied()),
            ("gap", p.gap),
            ("padding", p.padding),
            ("marT", p.mar_t),
            ("marR", p.mar_r),
            ("marB", p.mar_b),
            ("marL", p.mar_l),
            ("padT", p.pad_t.flatten()),
            ("padR", p.pad_r.flatten()),
            ("padB", p.pad_b.flatten()),
            ("padL", p.pad_l.flatten()),
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
