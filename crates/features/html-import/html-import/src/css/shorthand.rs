//! Shorthands to the longhands the mapping reads, so that the cascade can order every property on its own (a later
//! `margin-top` must beat an earlier `margin`). Only what `docs/import-html.md` section 3 reads is expanded:
//! `margin`, `padding`, `inset`, `gap`, `overflow`, `border` (and `border-width`, `border-style`, `border-color`,
//! and the four sides), `border-radius`, `flex`, `font` and `background` (one colour and one image: a colour, `none`,
//! a `url()` or a gradient). Anything else of a shorthand that this does not read is refused as a whole, never
//! guessed. A CSS-wide keyword (`inherit`, `initial`, `unset`, `revert`) goes to every longhand.

use super::value::{has_top_level_comma, is_wide, length, words};

/// Why a shorthand is not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refused(pub &'static str);

type Out = Result<Vec<(&'static str, String)>, Refused>;

const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];
const MARGIN: [&str; 4] = ["margin-top", "margin-right", "margin-bottom", "margin-left"];
const PADDING: [&str; 4] = [
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
];
const INSET: [&str; 4] = ["top", "right", "bottom", "left"];
const RADIUS: [&str; 4] = [
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-right-radius",
    "border-bottom-left-radius",
];
const WIDTH: [&str; 4] = [
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "border-left-width",
];
const STYLE: [&str; 4] = [
    "border-top-style",
    "border-right-style",
    "border-bottom-style",
    "border-left-style",
];
const COLOR: [&str; 4] = [
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
];
const FLEX: [&str; 3] = ["flex-grow", "flex-shrink", "flex-basis"];
const BG: [&str; 2] = ["background-color", "background-image"];
const FONT: [&str; 5] = [
    "font-style",
    "font-weight",
    "font-size",
    "line-height",
    "font-family",
];
const GAP: [&str; 2] = ["row-gap", "column-gap"];
const OVERFLOW: [&str; 2] = ["overflow-x", "overflow-y"];

/// The longhands of `name`, in a fixed order, when it is a shorthand this reads.
pub fn longhands(name: &str) -> Option<Vec<&'static str>> {
    let all =
        |parts: &[&[&'static str]]| Some(parts.iter().flat_map(|p| p.iter().copied()).collect());
    match name {
        "margin" => all(&[&MARGIN]),
        "padding" => all(&[&PADDING]),
        "inset" => all(&[&INSET]),
        "border-radius" => all(&[&RADIUS]),
        "border-width" => all(&[&WIDTH]),
        "border-style" => all(&[&STYLE]),
        "border-color" => all(&[&COLOR]),
        "border" => all(&[&WIDTH, &STYLE, &COLOR]),
        "border-top" | "border-right" | "border-bottom" | "border-left" => {
            let i = SIDES.iter().position(|s| name.ends_with(s))?;
            Some(vec![WIDTH[i], STYLE[i], COLOR[i]])
        }
        "flex" => all(&[&FLEX]),
        "font" => all(&[&FONT]),
        "background" => all(&[&BG]),
        "gap" => all(&[&GAP]),
        "overflow" => all(&[&OVERFLOW]),
        _ => None,
    }
}

/// One to four values as the four sides, top first (the CSS rule: a missing one is the opposite one, or the first).
fn four(v: &[&str]) -> Option<[String; 4]> {
    let s = |i: usize| v[i].to_owned();
    match v.len() {
        1 => Some([s(0), s(0), s(0), s(0)]),
        2 => Some([s(0), s(1), s(0), s(1)]),
        3 => Some([s(0), s(1), s(2), s(1)]),
        4 => Some([s(0), s(1), s(2), s(3)]),
        _ => None,
    }
}

fn pair(names: [&'static str; 4], v: [String; 4]) -> Vec<(&'static str, String)> {
    names.into_iter().zip(v).collect()
}

fn is_style(w: &str) -> bool {
    matches!(
        w.to_ascii_lowercase().as_str(),
        "none"
            | "hidden"
            | "dotted"
            | "dashed"
            | "solid"
            | "double"
            | "groove"
            | "ridge"
            | "inset"
            | "outset"
    )
}

fn is_width(w: &str) -> bool {
    matches!(w.to_ascii_lowercase().as_str(), "thin" | "medium" | "thick")
        || length(w, 16.0).is_some()
}

/// `width style color` in any order, each at most once; what is missing is the initial value.
fn border_parts(value: &str) -> Result<[String; 3], Refused> {
    let (mut w, mut s, mut c) = (None, None, None);
    let list = words(value);
    if list.is_empty() || list.len() > 3 {
        return Err(Refused("border_shorthand"));
    }
    for word in list {
        let slot = if is_style(word) {
            &mut s
        } else if is_width(word) {
            &mut w
        } else {
            &mut c
        };
        if slot.replace(word.to_owned()).is_some() {
            return Err(Refused("border_shorthand"));
        }
    }
    Ok([
        w.unwrap_or_else(|| "medium".into()),
        s.unwrap_or_else(|| "none".into()),
        c.unwrap_or_else(|| "currentcolor".into()),
    ])
}

fn flex(value: &str) -> Out {
    let w = words(value);
    let num = |s: &str| s.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0);
    let keyword = |g: &str, s: &str, b: &str| {
        Ok(vec![
            (FLEX[0], g.into()),
            (FLEX[1], s.into()),
            (FLEX[2], b.into()),
        ])
    };
    match w.as_slice() {
        [k] if k.eq_ignore_ascii_case("none") => keyword("0", "0", "auto"),
        [k] if k.eq_ignore_ascii_case("auto") => keyword("1", "1", "auto"),
        [g] if num(g).is_some() => keyword(g, "1", "0%"),
        [b] => keyword("1", "1", b),
        [g, s] if num(g).is_some() && num(s).is_some() => keyword(g, s, "0%"),
        [g, b] if num(g).is_some() => keyword(g, "1", b),
        [g, s, b] if num(g).is_some() && num(s).is_some() => keyword(g, s, b),
        _ => Err(Refused("flex_shorthand")),
    }
}

fn font(value: &str) -> Out {
    // `12px/1.5` and `12px / 1.5` are the same: the slash joins the words around it.
    let joined = value
        .replace(" / ", "/")
        .replace("/ ", "/")
        .replace(" /", "/");
    let w = words(&joined);
    let (mut style, mut weight) = ("normal".to_owned(), "normal".to_owned());
    for (i, word) in w.iter().enumerate() {
        let l = word.to_ascii_lowercase();
        if matches!(l.as_str(), "italic" | "oblique") {
            style = l;
        } else if matches!(l.as_str(), "bold" | "bolder" | "lighter")
            || l.parse::<u16>().is_ok_and(|n| (1..=1000).contains(&n))
        {
            weight = l;
        } else if matches!(l.as_str(), "normal" | "small-caps") {
        } else {
            let (size, lh) = word.split_once('/').unwrap_or((word, "normal"));
            let family = w[i + 1..].join(" ");
            if family.is_empty() || !(is_width(size) || size.ends_with('%')) {
                return Err(Refused("font_shorthand"));
            }
            return Ok(vec![
                (FONT[0], style),
                (FONT[1], weight),
                (FONT[2], size.to_owned()),
                (FONT[3], lh.to_owned()),
                (FONT[4], family),
            ]);
        }
    }
    Err(Refused("font_shorthand"))
}

fn background(value: &str) -> Out {
    let (mut color, mut image) = (None, None);
    let list = words(value);
    if list.is_empty() || has_top_level_comma(value) {
        return Err(Refused("background_shorthand"));
    }
    for word in list {
        let l = word.to_ascii_lowercase();
        let is_image = l == "none" || l.starts_with("url(") || l.contains("-gradient(");
        let slot = if is_image { &mut image } else { &mut color };
        if slot.replace(word.to_owned()).is_some() {
            return Err(Refused("background_shorthand"));
        }
    }
    Ok(vec![
        (BG[0], color.unwrap_or_else(|| "transparent".into())),
        (BG[1], image.unwrap_or_else(|| "none".into())),
    ])
}

/// Expand `name: value`. `None`: `name` is not a shorthand this reads. `Some(Err)`: it is, and this value is not
/// read; nothing of it is kept.
pub fn expand(name: &str, value: &str) -> Option<Out> {
    let names = longhands(name)?;
    let value = value.trim();
    if is_wide(value) {
        return Some(Ok(names
            .into_iter()
            .map(|n| (n, value.to_owned()))
            .collect()));
    }
    let w = words(value);
    let sides = |names: [&'static str; 4]| {
        four(&w)
            .map(|v| pair(names, v))
            .ok_or(Refused("too_many_values"))
    };
    Some(match name {
        "margin" => sides(MARGIN),
        "padding" => sides(PADDING),
        "inset" => sides(INSET),
        "border-width" => sides(WIDTH),
        "border-style" => sides(STYLE),
        "border-color" => sides(COLOR),
        "border-radius" if value.contains('/') => Err(Refused("elliptical_radius")),
        "border-radius" => sides(RADIUS),
        "gap" => match w.as_slice() {
            [a] => Ok(vec![(GAP[0], (*a).into()), (GAP[1], (*a).into())]),
            [a, b] => Ok(vec![(GAP[0], (*a).into()), (GAP[1], (*b).into())]),
            _ => Err(Refused("too_many_values")),
        },
        "overflow" => match w.as_slice() {
            [a] => Ok(vec![(OVERFLOW[0], (*a).into()), (OVERFLOW[1], (*a).into())]),
            [a, b] => Ok(vec![(OVERFLOW[0], (*a).into()), (OVERFLOW[1], (*b).into())]),
            _ => Err(Refused("too_many_values")),
        },
        "border" => border_parts(value).map(|[wd, st, co]| {
            let mut out = pair(WIDTH, std::array::from_fn(|_| wd.clone()));
            out.extend(pair(STYLE, std::array::from_fn(|_| st.clone())));
            out.extend(pair(COLOR, std::array::from_fn(|_| co.clone())));
            out
        }),
        "flex" => flex(value),
        "font" => font(value),
        "background" => background(value),
        side => border_parts(value).map(|[wd, st, co]| {
            // `border-top` and its three siblings.
            let i = SIDES.iter().position(|s| side.ends_with(s)).unwrap_or(0);
            vec![(WIDTH[i], wd), (STYLE[i], st), (COLOR[i], co)]
        }),
    })
}
