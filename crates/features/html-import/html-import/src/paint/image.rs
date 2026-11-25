//! A `url()` layer of `background-image`. The model's `image` is an asset id of the workspace, and nothing is
//! fetched, so one form is read: the one the export writes, `url(<base><64 hex>)` with `background-size` `cover`,
//! `contain` or `100% 100%` (the model's three fits), `background-position: center` and `background-repeat:
//! no-repeat`. The base is whatever the export was given and ends in `/` (or is empty). Every other `url()`, and an
//! asset url whose size, position or repeat is not the export's, is a placeholder: not carried, its text kept under
//! `assets`. A `data:` url keeps only its media type; an SVG is refused until sanitisation exists.

use super::lex::{inner, split_commas};
use super::{Asset, Paint, Sink};
use crate::css::Style;
use crate::css::value::words;
use meldframe_protocol::{ImageFill, ImageFit};

/// The text inside `url(...)`, without quotes.
fn src(layer: &str) -> &str {
    let s = inner(layer, "url").unwrap_or("").trim();
    for q in ['"', '\''] {
        if let Some(t) = s.strip_prefix(q).and_then(|t| t.strip_suffix(q)) {
            return t.trim();
        }
    }
    s
}

/// The asset id at the end of the url: the last path segment, 64 lower-case hex digits.
fn asset_id(src: &str) -> Option<&str> {
    let tail = src.rsplit('/').next()?;
    (tail.len() == 64 && tail.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .then_some(tail)
}

/// The `i`th value of a comma list property, repeated as CSS does when the list is shorter; none when the property is
/// not declared.
fn nth(style: &Style, name: &str, i: usize) -> Option<String> {
    let (list, _) = split_commas(style.get(name)?, 16);
    let v = list.get(i % list.len().max(1))?;
    Some(words(v).join(" ").to_ascii_lowercase())
}

fn fit(size: Option<&str>) -> Option<ImageFit> {
    match size? {
        "cover" => Some(ImageFit::Cover),
        "contain" => Some(ImageFit::Contain),
        "100% 100%" => Some(ImageFit::Fill),
        _ => None,
    }
}

/// Read the `url()` layer number `i`.
pub(super) fn read(layer: &str, i: usize, style: &Style, out: &mut Paint, k: &mut Sink) {
    let text = src(layer);
    let lower = text.to_ascii_lowercase();
    let path = lower.split(['?', '#']).next().unwrap_or("");
    let placeholder = |out: &mut Paint| out.assets.push(Asset::new(text));
    if text.is_empty() {
        k.refuse("background_url", "background-image", layer, "an empty url");
        return;
    }
    if lower.starts_with("data:") {
        k.refuse(
            "image_data_uri",
            "background-image",
            layer,
            "a data: image is not decoded in this version",
        );
        placeholder(out);
        return;
    }
    if path.ends_with(".svg") {
        k.refuse(
            "image_svg",
            "background-image",
            layer,
            "an SVG is not read until it can be sanitised",
        );
        placeholder(out);
        return;
    }
    let Some(id) = asset_id(text) else {
        k.refuse(
            "background_url",
            "background-image",
            layer,
            "not fetched: upload the file as an asset and set the image of the node",
        );
        placeholder(out);
        return;
    };
    let center =
        |p: Option<String>| matches!(p.as_deref(), Some("center" | "center center" | "50% 50%"));
    let no_repeat =
        |r: Option<String>| matches!(r.as_deref(), Some("no-repeat" | "no-repeat no-repeat"));
    let size = fit(nth(style, "background-size", i).as_deref());
    let problem = if size.is_none() {
        Some((
            "background_size",
            "the size is not cover, contain or 100% 100%",
        ))
    } else if !center(nth(style, "background-position", i)) {
        Some(("background_position", "the position is not center"))
    } else if !no_repeat(nth(style, "background-repeat", i)) {
        Some(("background_repeat", "the repeat is not no-repeat"))
    } else {
        None
    };
    if let Some((code, why)) = problem {
        k.refuse(code, "background-image", layer, why);
        placeholder(out);
    } else if out.image.is_some() {
        k.refuse(
            "background_layer_extra",
            "background-image",
            layer,
            "the model holds one image: the first is kept",
        );
        placeholder(out);
    } else {
        out.image = Some(ImageFill {
            asset_id: id.to_owned(),
            fit: size.unwrap_or_default(),
        });
    }
}
