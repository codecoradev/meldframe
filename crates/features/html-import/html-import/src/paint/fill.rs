//! The fill: `background-color`, and the layers of `background-image` (a gradient and, in a later step, an image).
//! The model holds one colour, one gradient over it and one image over that.

use super::gradient;
use super::lex::{colour_word, inner, split_commas};
use super::{Ctx, Paint, Sink};
use crate::css::Style;
use crate::css::value::words;
use meldframe_protocol::Bind;

/// The most layers of one `background-image` that are looked at.
const MAX_LAYERS: usize = 16;

pub(super) fn read(style: &Style, ctx: &Ctx, out: &mut Paint, k: &mut Sink) {
    colour(style, out, k);
    layers(style, ctx, out, k);
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

/// The name of the call that `layer` is, when it is exactly one call (`name(...)`).
fn call_name(layer: &str) -> Option<&str> {
    let name = &layer[..layer.find('(')?];
    let ident = !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-');
    (ident && inner(layer, name).is_some()).then_some(name)
}

/// Does a `background-size` make a gradient fill the box (a gradient has no size of its own).
fn fills_the_box(size: &str) -> bool {
    let w: Vec<String> = words(size).iter().map(|s| s.to_ascii_lowercase()).collect();
    matches!(
        w.join(" ").as_str(),
        "auto" | "auto auto" | "100%" | "100% 100%" | "cover" | "contain"
    )
}

fn layers(style: &Style, ctx: &Ctx, out: &mut Paint, k: &mut Sink) {
    let Some(value) = style.get("background-image").map(str::trim) else {
        return;
    };
    if value.eq_ignore_ascii_case("none") {
        return;
    }
    let (list, cut) = split_commas(value, MAX_LAYERS);
    if cut {
        k.cut(
            "background_layers_cut",
            "background-image",
            value,
            "more than 16 layers: the rest are not read",
        );
    }
    let sizes = split_commas(style.get("background-size").unwrap_or(""), MAX_LAYERS).0;
    for (i, layer) in list.iter().enumerate() {
        k.steps += 1 + layer.len() / 64;
        let name = call_name(layer).map(str::to_ascii_lowercase);
        match name.as_deref() {
            Some(n @ ("linear-gradient" | "radial-gradient")) => {
                if let Some(size) = sizes.get(i % sizes.len().max(1))
                    && !fills_the_box(size)
                {
                    k.refuse("background_size", "background-image", layer, "a gradient with a size of its own is a pattern, which the model has no field for");
                    continue;
                }
                match gradient::parse(n, layer, style, ctx, k) {
                    Ok(_) if out.gradient.is_some() => k.refuse(
                        "background_layer_extra",
                        "background-image",
                        layer,
                        "the model holds one gradient: the first is kept",
                    ),
                    Ok(g) => out.gradient = Some(g),
                    Err((code, why)) => k.refuse(code, "background-image", layer, why),
                }
            }
            Some(
                "conic-gradient"
                | "repeating-linear-gradient"
                | "repeating-radial-gradient"
                | "repeating-conic-gradient",
            ) => {
                k.refuse(
                    "gradient_kind",
                    "background-image",
                    layer,
                    "only linear-gradient and radial-gradient are carried",
                );
            }
            _ => k.refuse(
                "background_image_unreadable",
                "background-image",
                layer,
                "a layer this does not read",
            ),
        }
    }
}
