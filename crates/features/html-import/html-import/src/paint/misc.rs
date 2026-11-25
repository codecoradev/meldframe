//! `opacity`.

use super::lex::{number, percent};
use super::{Paint, Sink};
use crate::css::Style;
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
