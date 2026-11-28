//! `box-shadow`: the model holds up to 4 outer shadows (an offset, a blur, a spread and a colour), the first of its
//! list drawn at the bottom; CSS draws the first at the top, so the list is reversed. An inner shadow has no field
//! and is refused; of more than four the four topmost (the first four of the CSS list) are kept.

use super::lex::{colour_word, split_commas};
use super::{Paint, Sink};
use crate::css::Style;
use crate::css::value::{length, words};
use meldframe_css::number::starts_like_number;
use meldframe_protocol::Shadow;

/// The most shadows the model holds.
const MAX_SHADOWS: usize = 4;
/// The most layers of one list that are looked at.
const MAX_LAYERS: usize = 16;
/// The largest offset, blur or spread the model checks.
const LIMIT: f64 = 100_000.0;

/// One layer. `Ok(None)` is an inner shadow.
fn one(layer: &str, style: &Style) -> Result<Option<Shadow>, &'static str> {
    let bad = Err("not offsets and a colour: two to four lengths and at most one colour");
    let mut w = words(layer);
    let before = w.len();
    w.retain(|x| !x.eq_ignore_ascii_case("inset"));
    let inset = match before - w.len() {
        0 => false,
        1 => true,
        _ => return bad,
    };
    let is_len: Vec<bool> = w.iter().map(|x| starts_like_number(x)).collect();
    let n = is_len.iter().filter(|l| **l).count();
    // The colour is the one word that is not a length, and it comes first or last.
    let (lengths, colour): (Vec<&str>, Option<&str>) =
        match (w.len() - n, is_len.first(), is_len.last()) {
            (0, _, _) => (w.clone(), None),
            (1, Some(false), _) => (w[1..].to_vec(), Some(w[0])),
            (1, _, Some(false)) => (w[..w.len() - 1].to_vec(), Some(w[w.len() - 1])),
            _ => return bad,
        };
    if !(2..=4).contains(&lengths.len()) {
        return bad;
    }
    let mut px = [0.0f64; 4];
    for (slot, l) in px.iter_mut().zip(&lengths) {
        match length(l, style.font_size, style.rem) {
            Some(v) if v.px.abs() <= LIMIT => *slot = v.px,
            _ => return Err("an offset, blur or spread that is not a length in range"),
        }
    }
    if px[2] < 0.0 {
        return Err("a negative blur");
    }
    let color = match colour {
        None => style.color.clone(),
        Some(c) => colour_word(c, style).ok_or("a colour that is not read")?.0,
    };
    if inset {
        return Ok(None);
    }
    Ok(Some(Shadow {
        x: px[0],
        y: px[1],
        blur: px[2],
        spread: px[3],
        color,
    }))
}

pub(super) fn read(style: &Style, out: &mut Paint, k: &mut Sink) {
    let Some(v) = style.get("box-shadow").map(str::trim) else {
        return;
    };
    if v.eq_ignore_ascii_case("none") {
        return;
    }
    let (layers, cut) = split_commas(v, MAX_LAYERS);
    if cut {
        k.cut(
            "shadow_layers_cut",
            "box-shadow",
            v,
            "more than 16 shadows: the rest are not read",
        );
    }
    let mut list = Vec::new();
    for layer in layers {
        k.steps += 1 + layer.len() / 64;
        match one(layer, style) {
            Ok(Some(_)) if list.len() >= MAX_SHADOWS => k.refuse(
                "shadow_extra",
                "box-shadow",
                layer,
                "the model holds 4 shadows: the four topmost are kept",
            ),
            Ok(Some(s)) => list.push(s),
            Ok(None) => k.refuse(
                "shadow_inset",
                "box-shadow",
                layer,
                "inner shadows have no field",
            ),
            Err(why) => k.refuse("shadow_unreadable", "box-shadow", layer, why),
        }
    }
    list.reverse();
    out.shadows = list;
}
