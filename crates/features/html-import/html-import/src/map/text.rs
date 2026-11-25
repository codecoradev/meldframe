//! The text of an element: the pieces of its content, joined as the browser would show them, and the typography the
//! model keeps (`docs/import-html.md` section 3.5).

use super::item::{Approx, Notes};
use super::pre::is_space;
use crate::css::Style;
use meldframe_protocol::{Bind, Node, TextAlign};

/// The most a font size may be (`meldframe_check::Limits::max_size`).
const MAX_FONT: f64 = 100_000.0;

/// A piece of the content of a line of text.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Piece<'a> {
    Text(&'a str),
    /// A `<br>`.
    Br,
}

/// The pieces of one line of text and the first thing an inline element in it did differently.
#[derive(Debug, Default)]
pub(crate) struct Run<'a> {
    pub pieces: Vec<Piece<'a>>,
    pub diff: Option<Approx>,
}

impl<'a> Run<'a> {
    pub fn of(text: &'a str) -> Self {
        Run {
            pieces: vec![Piece::Text(text)],
            diff: None,
        }
    }
}

/// What `white-space` does to the white space of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Space {
    /// `normal`, `nowrap`: runs of white space are one space, the line ends are spaces.
    Collapse,
    /// `pre-line`: runs of spaces are one, the line ends stay.
    Lines,
    /// `pre`, `pre-wrap`: all of it stays.
    Keep,
}

/// How `white-space` of the element reads. Another value is refused and the text collapses.
pub(crate) fn space(st: &Style, notes: &mut Notes) -> Space {
    match st.get("white-space").map(|v| v.trim().to_ascii_lowercase()) {
        None => Space::Collapse,
        Some(v) => match v.as_str() {
            "normal" | "nowrap" => Space::Collapse,
            "pre-line" => Space::Lines,
            "pre" | "pre-wrap" => Space::Keep,
            _ => {
                notes.refuse("white-space", &v, "refused_value");
                Space::Collapse
            }
        },
    }
}

/// The pieces as one string. With `Collapse` a run of spaces, tabs and line ends is one space and a space at the
/// start or the end of a line goes; a `<br>` is a line end that stays in every mode.
pub(crate) fn collapse(pieces: &[Piece], how: Space) -> String {
    let mut out = String::new();
    let (mut space, mut start) = (false, true);
    for p in pieces {
        match p {
            Piece::Br => {
                (space, start) = (false, true);
                out.push('\n');
            }
            Piece::Text(t) if how == Space::Keep => out.push_str(t),
            Piece::Text(t) => {
                for c in t.chars() {
                    if c == '\n' && how == Space::Lines {
                        (space, start) = (false, true);
                        out.push('\n');
                    } else if is_space(c) {
                        space = !start;
                    } else {
                        if space {
                            out.push(' ');
                        }
                        space = false;
                        start = false;
                        out.push(c);
                    }
                }
            }
        }
    }
    if how != Space::Keep {
        // A `<br>` at the end of a block does not make another line.
        out.truncate(out.trim_end_matches('\n').len());
    }
    out
}

/// One text with the white space collapsed.
pub(crate) fn collapse_plain(t: &str) -> String {
    collapse(&[Piece::Text(t)], Space::Collapse)
}

/// `text-transform` applied to the characters: the style is lost, the look is kept.
pub(crate) fn transform(text: String, st: &Style, notes: &mut Notes) -> String {
    let Some(v) = st
        .get("text-transform")
        .map(|v| v.trim().to_ascii_lowercase())
    else {
        return text;
    };
    let out = match v.as_str() {
        "none" => return text,
        "uppercase" => text.to_uppercase(),
        "lowercase" => text.to_lowercase(),
        "capitalize" => {
            let mut start = true;
            text.chars()
                .flat_map(|c| {
                    let up = start && c.is_alphanumeric();
                    start = c.is_whitespace() || (start && !c.is_alphanumeric());
                    if up {
                        c.to_uppercase().collect::<Vec<_>>()
                    } else {
                        vec![c]
                    }
                })
                .collect()
        }
        _ => {
            notes.refuse("text-transform", &v, "refused_value");
            return text;
        }
    };
    notes.approximate("text-transform", &v, "applied to the characters");
    out
}

/// The size, weight, colour and alignment of a text. The size is kept to the model's bounds and the weight to the
/// nearest step of 100; `start`, `end` and `justify` are read left to right.
pub(crate) fn typography(node: &mut Node, st: &Style, notes: &mut Notes) {
    node.fill = Bind::Lit(st.color.clone());
    node.font_size = st.font_size.clamp(1.0, MAX_FONT);
    if node.font_size != st.font_size {
        notes.approximate(
            "font-size",
            &format!("{}px", st.font_size),
            &format!("{}px", node.font_size),
        );
    }
    let step = ((st.font_weight / 100.0).round() as u16).clamp(1, 9) * 100;
    node.font_weight = step;
    if f64::from(step) != st.font_weight {
        notes.approximate(
            "font-weight",
            &st.font_weight.to_string(),
            &step.to_string(),
        );
    }
    if let Some(v) = st.get("text-align").map(|v| v.trim().to_ascii_lowercase()) {
        let (align, approx) = match v.as_str() {
            "left" => (TextAlign::Left, false),
            "center" => (TextAlign::Center, false),
            "right" => (TextAlign::Right, false),
            "start" | "justify" => (TextAlign::Left, true),
            "end" => (TextAlign::Right, true),
            _ => {
                notes.refuse("text-align", &v, "refused_value");
                (TextAlign::Left, false)
            }
        };
        node.text_align = align;
        if approx {
            let to = if align == TextAlign::Left {
                "left"
            } else {
                "right"
            };
            notes.approximate("text-align", &v, to);
        }
    }
}

/// The first of the size, the weight and the colour in which an inline element differs from the text it is joined
/// into: the model has one style for a text, so the others are lost.
pub(crate) fn first_difference(text: &Style, inline: &Style) -> Option<Approx> {
    let pair = |property: &str, from: String, to: String| {
        (from != to).then(|| Approx {
            property: property.into(),
            from,
            to,
        })
    };
    pair(
        "font-size",
        format!("{}px", inline.font_size),
        format!("{}px", text.font_size),
    )
    .or_else(|| {
        pair(
            "font-weight",
            inline.font_weight.to_string(),
            text.font_weight.to_string(),
        )
    })
    .or_else(|| pair("color", inline.color.clone(), text.color.clone()))
}

/// A tab, a URL or CJK text: the browser and the canvas break it differently (contract, section 4.3).
pub(crate) fn outside_contract(t: &str) -> bool {
    t.contains('\t')
        || t.contains("://")
        || t.chars().any(|c| {
            ('\u{3000}'..='\u{9fff}').contains(&c) || ('\u{ac00}'..='\u{d7af}').contains(&c)
        })
}
