//! The fill: `background-color`. The layers of `background-image` come in the next steps.

use super::lex::colour_word;
use super::{Ctx, Paint, Sink};
use crate::css::Style;
use meldframe_protocol::Bind;

pub(super) fn read(style: &Style, _ctx: &Ctx, out: &mut Paint, k: &mut Sink) {
    colour(style, out, k);
}

/// `background-color`: a colour with its alpha, or none for `transparent` and for no declaration.
fn colour(style: &Style, out: &mut Paint, k: &mut Sink) {
    let Some(v) = style.get("background-color").map(str::trim) else {
        return;
    };
    if v.eq_ignore_ascii_case("transparent") {
        return;
    }
    k.steps += 1;
    match colour_word(v, style) {
        Some((hex, clipped)) => {
            if clipped {
                k.list(
                    "colour_clipped",
                    "background-color",
                    v,
                    "the colour is outside sRGB and was clamped",
                );
            }
            out.fill = Bind::Lit(hex);
        }
        None => k.refuse(
            "colour_unreadable",
            "background-color",
            v,
            "not a colour this reads",
        ),
    }
}
