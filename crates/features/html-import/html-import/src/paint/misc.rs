//! `opacity`, and the properties of paint that have no field in the model (and the ones of behaviour), which are
//! refused by name.

use super::lex::{number, percent};
use super::{Paint, Sink};
use crate::css::{Source, Style};
use meldframe_protocol::Bind;

/// `opacity`: a number or a percentage from 0 to 1. CSS clamps a value outside it; the model refuses one, so it is
/// clamped and listed.
pub(super) fn opacity(style: &Style, out: &mut Paint, k: &mut Sink) {
    let Some(v) = style.get("opacity").map(str::trim) else {
        return;
    };
    k.steps += 1;
    match percent(v).map(|p| p / 100.0).or_else(|| number(v)) {
        Some(x) if (0.0..=1.0).contains(&x) => out.opacity = Bind::Lit(x),
        Some(x) => {
            k.list(
                "opacity_clamped",
                "opacity",
                v,
                "outside 0 to 1: clamped, as CSS does",
            );
            out.opacity = Bind::Lit(x.clamp(0.0, 1.0));
        }
        None => k.refuse(
            "opacity_unreadable",
            "opacity",
            v,
            "not a number or a percentage",
        ),
    }
}

/// How a refused property is told: a line of its own (paint that is lost), or a count by code (behaviour).
#[derive(Clone, Copy)]
enum Kind {
    Visual,
    Behaviour(&'static str),
}

/// What the table says of a name: the values that mean nothing (the initial value) and the kind.
fn rule(name: &str) -> Option<(&'static [&'static str], Kind)> {
    use Kind::{Behaviour, Visual};
    Some(match name {
        "background-clip" | "-webkit-background-clip" => (&["border-box"], Visual),
        "background-attachment" => (&["scroll"], Visual),
        "background-blend-mode" => (&["normal"], Visual),
        "background-origin" => (&["padding-box"], Visual),
        "mix-blend-mode" => (&["normal"], Visual),
        "isolation" => (&["auto"], Visual),
        "filter" | "backdrop-filter" | "-webkit-backdrop-filter" | "clip-path" => {
            (&["none"], Visual)
        }
        "outline" => (&["none", "0"], Visual),
        "outline-style" => (&["none"], Visual),
        "border-image" | "border-image-source" => (&["none"], Visual),
        "cursor" => (&["auto"], Behaviour("behaviour_cursor")),
        "pointer-events" => (&["auto"], Behaviour("behaviour_pointer_events")),
        "user-select" | "-webkit-user-select" => (&["auto"], Behaviour("behaviour_user_select")),
        "will-change" => (&["auto"], Behaviour("behaviour_will_change")),
        n if n.starts_with("outline-") => (&[], Visual),
        n if n.starts_with("mask") || n.starts_with("-webkit-mask") => (&["none"], Visual),
        n if n.starts_with("transition") => (&["none"], Behaviour("behaviour_transition")),
        n if n.starts_with("animation") => (&["none"], Behaviour("behaviour_animation")),
        // A logical border (`border-block`, `border-inline-start-width`): not expanded, not read.
        n if n.starts_with("border-block")
            || n.starts_with("border-inline")
            || n.starts_with("border-start")
            || n.starts_with("border-end") =>
        {
            (&[], Visual)
        }
        _ => return None,
    })
}

/// Say what of the declared paint properties has no field. Only what the author wrote: the browser's defaults and
/// the values an element inherits are not counted again.
pub(super) fn refused(style: &Style, k: &mut Sink) {
    for (name, prop) in style.declared() {
        if !matches!(prop.source, Source::Author | Source::Inline) {
            continue;
        }
        let Some((initial, kind)) = rule(name) else {
            continue;
        };
        let v = prop.value.trim();
        k.steps += 1;
        if initial.iter().any(|i| v.eq_ignore_ascii_case(i)) {
            continue;
        }
        match kind {
            Kind::Visual => k.refuse("refused_not_modelled", name, v, "no field in the model"),
            Kind::Behaviour(code) => k.report.count(code, 1),
        }
    }
}

/// Does this stage have an answer for the property `name`: it reads it, or it refuses it by name. Another stage
/// that lists the properties it does not read leaves these out, so that none is said twice.
pub fn handles(name: &str) -> bool {
    matches!(
        name,
        "background-color"
            | "background-image"
            | "background-size"
            | "background-position"
            | "background-repeat"
            | "box-shadow"
            | "opacity"
    ) || (name.starts_with("border-") && !matches!(name, "border-collapse" | "border-spacing"))
        || name == "border"
        || rule(name).is_some()
}
