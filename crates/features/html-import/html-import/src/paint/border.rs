//! The border. The model's border is paint only: a width, a colour and a position (`inside`, `center`, `outside`),
//! drawn on the box and taking no room. CSS has two forms that map to it:
//!
//! - the element's own `border`: four equal solid sides. It takes room in CSS, so the paint says how much
//!   ([`Paint::border_room`]) and the structure stage adds that to the padding. The position is `inside`;
//! - the export's border, the `::after` box (`content: ""`, `position: absolute`, an `inset` of 0, `-w/2` or `-w`
//!   for `inside`, `center` and `outside`, and `border: w solid c`). It takes no room: this is the round trip.
//!
//! A border with sides that differ, a style other than solid, an inset that is none of the three, or a generated
//! box that is not that form, has no field in the model and is refused.

use super::lex::colour_word;
use super::{Paint, Sink};
use crate::css::value::length;
use crate::css::{Props, Style};
use meldframe_protocol::{Border, BorderPosition};

const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];
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
/// The widest border the model checks.
const MAX_WIDTH: f64 = 100_000.0;

#[derive(Debug, PartialEq)]
enum Side {
    Off,
    On(f64, String),
    Bad(&'static str, &'static str),
}

fn get<'a>(p: &'a Props, name: &str) -> Option<&'a str> {
    p.get(name).map(|v| v.value.trim())
}

/// One side: its width in pixels and colour when it is drawn. `keyword` is set when the width was a keyword or
/// left out, which is a width the model reads from a table.
fn side(p: &Props, style: &Style, i: usize, keyword: &mut bool) -> Side {
    let line = get(p, STYLE[i]).unwrap_or("none").to_ascii_lowercase();
    if line == "none" || line == "hidden" {
        return Side::Off;
    }
    if line != "solid" {
        return Side::Bad(
            "border_style",
            "only solid is carried: dashed, dotted and the others have no field",
        );
    }
    let width = match get(p, WIDTH[i]).map(str::to_ascii_lowercase).as_deref() {
        None | Some("medium") => {
            *keyword = true;
            3.0
        }
        Some("thin") => {
            *keyword = true;
            1.0
        }
        Some("thick") => {
            *keyword = true;
            5.0
        }
        Some(w) => match length(w, style.font_size, style.rem)
            .map(|l| l.px)
            .filter(|px| (0.0..=MAX_WIDTH).contains(px))
        {
            Some(px) => px,
            None => {
                return Side::Bad(
                    "border_width_unreadable",
                    "a width that is not a length in pixels, rem, em or pt",
                );
            }
        },
    };
    if width == 0.0 {
        return Side::Off;
    }
    match get(p, COLOR[i]) {
        None => Side::On(width, style.color.clone()),
        Some(c) => match colour_word(c, style) {
            Some((hex, _)) => Side::On(width, hex),
            None => Side::Bad("colour_unreadable", "not a colour this reads"),
        },
    }
}

/// The border of the four sides in `p`, when they are the same.
fn four_sides(p: &Props, style: &Style, k: &mut Sink) -> Option<Border> {
    let mut keyword = false;
    let sides: [Side; 4] = std::array::from_fn(|i| side(p, style, i, &mut keyword));
    k.steps += 4;
    let summary = || {
        format!(
            "{} {} {}",
            get(p, WIDTH[0]).unwrap_or(""),
            get(p, STYLE[0]).unwrap_or(""),
            get(p, COLOR[0]).unwrap_or("")
        )
    };
    if sides.iter().any(|s| *s != sides[0]) {
        k.refuse(
            "border_per_side",
            "border",
            &summary(),
            "sides that differ have no field: the model has one border",
        );
        return None;
    }
    match &sides[0] {
        Side::Off => None,
        Side::Bad(code, why) => {
            k.refuse(code, "border", &summary(), why);
            None
        }
        Side::On(width, color) => {
            if keyword {
                k.list(
                    "border_width_keyword",
                    "border",
                    &summary(),
                    "thin, medium and thick are read as 1, 3 and 5 pixels, as browsers draw them",
                );
            }
            Some(Border {
                width: *width,
                color: color.clone(),
                position: BorderPosition::Inside,
            })
        }
    }
}

/// Does the generated box declare any border property.
fn has_border(a: &Props) -> bool {
    (0..4).any(|i| {
        [WIDTH[i], STYLE[i], COLOR[i]]
            .iter()
            .any(|n| a.contains_key(*n))
    })
}

/// A property of the generated box that the export's border form has.
fn in_form(name: &str) -> bool {
    matches!(
        name,
        "content"
            | "position"
            | "pointer-events"
            | "box-sizing"
            | "top"
            | "right"
            | "bottom"
            | "left"
    ) || (name.starts_with("border-")
        && (name.ends_with("-width")
            || name.ends_with("-style")
            || name.ends_with("-color")
            || name.ends_with("-radius")))
}

/// The border of the export, read from the `::after` box.
fn export_box(style: &Style, k: &mut Sink) -> Option<Border> {
    let a = style.after.as_ref().filter(|a| has_border(a))?;
    for (name, prop) in a.iter() {
        if !in_form(name) && prop.value.trim() != "none" {
            k.refuse(
                "after_property",
                name,
                &prop.value,
                "not read on the box of a border",
            );
        }
    }
    let content = get(a, "content").unwrap_or("");
    let form = (content == "\"\"" || content == "''")
        && get(a, "position").is_some_and(|p| p.eq_ignore_ascii_case("absolute"));
    if !form {
        k.refuse(
            "after_not_border_form",
            "::after",
            content,
            "a box that is not content \"\" with position absolute is not the border of the export",
        );
        return None;
    }
    let border = four_sides(a, style, k)?;
    // The four sides of the inset say where the border sits: on the edge, half out or all out.
    let inset: [Option<f64>; 4] = SIDES.map(|s| {
        get(a, s)
            .and_then(|v| length(v, style.font_size, style.rem))
            .map(|l| l.px)
    });
    let w = border.width;
    let same = |want: f64| {
        inset
            .iter()
            .all(|i| i.is_some_and(|i| (i - want).abs() < 1e-6))
    };
    let position = if same(0.0) {
        BorderPosition::Inside
    } else if same(-w / 2.0) {
        BorderPosition::Center
    } else if same(-w) {
        BorderPosition::Outside
    } else {
        k.refuse(
            "after_inset",
            "inset",
            get(a, "top").unwrap_or(""),
            "the inset is none of 0, -w/2 and -w, the three positions of the model",
        );
        return None;
    };
    Some(Border { position, ..border })
}

pub(super) fn read(style: &Style, out: &mut Paint, k: &mut Sink) {
    let own = four_sides(&style.props, style, k);
    match (own, export_box(style, k)) {
        (own, Some(b)) => {
            if own.is_some() {
                k.refuse(
                    "border_two_sources",
                    "border",
                    "",
                    "the element and its ::after box both draw a border: the model holds one, the box of the export is kept",
                );
            }
            out.border = b;
        }
        (Some(b), None) => {
            out.border_room = b.width;
            out.border = b;
        }
        (None, None) => {}
    }
}
