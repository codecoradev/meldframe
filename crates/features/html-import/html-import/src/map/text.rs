//! Text (`docs/import-html.md` section 3.5): the pieces of a run, how a browser collapses their spaces, and the style
//! a text node takes from its box. The text the HTML holds is file content: it is only collapsed and cased as the
//! source style asks, never cleaned of hidden characters (rule U9).

use super::notes::Notes;
use crate::css::Style;
use meldframe_protocol::TextAlign;

/// A piece of a run of text: characters, or the line break of a `br`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Piece {
    Text(String),
    Br,
}

/// How a browser treats white space (`white-space`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ws {
    Normal,
    Nowrap,
    Pre,
    PreWrap,
    PreLine,
}

impl Ws {
    pub fn of(s: &Style) -> Ws {
        match s
            .get("white-space")
            .map(|v| v.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("nowrap") => Ws::Nowrap,
            Some("pre") => Ws::Pre,
            Some("pre-wrap" | "break-spaces") => Ws::PreWrap,
            Some("pre-line") => Ws::PreLine,
            _ => Ws::Normal,
        }
    }

    /// The text does not wrap, so its box is as wide as the line: a hug width (contract 4.2: hug is `pre`).
    pub fn hugs(self) -> bool {
        matches!(self, Ws::Nowrap | Ws::Pre)
    }

    fn collapses(self) -> bool {
        matches!(self, Ws::Normal | Ws::Nowrap | Ws::PreLine)
    }
}

/// The white space a browser collapses (a no-break space is not one).
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}')
}

/// The pieces as the text of a node: spaces collapsed to one and trimmed at both ends unless `ws` keeps them, a `br`
/// a line break. Empty when nothing but white space is left.
pub(super) fn render(pieces: &[Piece], ws: Ws) -> String {
    let mut out = String::new();
    if ws.collapses() {
        let mut pending = false;
        for p in pieces {
            match p {
                Piece::Br => {
                    pending = false;
                    out.push('\n');
                }
                Piece::Text(t) => {
                    for c in t.chars() {
                        if c == '\n' && ws == Ws::PreLine {
                            pending = false;
                            out.push('\n');
                        } else if is_space(c) {
                            pending = true;
                        } else {
                            if pending && !out.is_empty() && !out.ends_with('\n') {
                                out.push(' ');
                            }
                            pending = false;
                            out.push(c);
                        }
                    }
                }
            }
        }
        let end = out.trim_end_matches('\n').len();
        out.truncate(end);
    } else {
        for p in pieces {
            match p {
                Piece::Br => out.push('\n'),
                Piece::Text(t) => out.push_str(t),
            }
        }
    }
    if out.chars().all(is_space) {
        out.clear();
    }
    out
}

/// `text-transform` applied to the characters.
fn transform(s: &str, how: &str) -> String {
    match how {
        "uppercase" => s.to_uppercase(),
        "lowercase" => s.to_lowercase(),
        "capitalize" => {
            let mut out = String::with_capacity(s.len());
            let mut start = true;
            for c in s.chars() {
                if start && c.is_alphabetic() {
                    out.extend(c.to_uppercase());
                } else {
                    out.push(c);
                }
                start = c.is_whitespace();
            }
            out
        }
        _ => s.to_owned(),
    }
}

/// What a text node takes from the style of the box that holds it: the properties that inherit, which are the ones
/// the model has a field for (contract 4.2).
#[derive(Debug, Clone)]
pub(super) struct TextStyle {
    pub size: f64,
    pub weight: u16,
    pub color: String,
    pub align: TextAlign,
    pub ws: Ws,
    transform: String,
    notes: Vec<(&'static str, String, String)>,
}

impl TextStyle {
    pub fn of(s: &Style) -> TextStyle {
        let mut notes = Vec::new();
        let weight = {
            let w = s.font_weight;
            let step = ((w / 100.0).round() * 100.0).clamp(100.0, 900.0);
            if (w - step).abs() > f64::EPSILON {
                notes.push(("font-weight", format!("{w}"), format!("{step}")));
            }
            step as u16
        };
        let get = |n: &str| s.get(n).map(|v| v.trim().to_ascii_lowercase());
        let align = match get("text-align").as_deref() {
            Some("center") => TextAlign::Center,
            Some("right") => TextAlign::Right,
            Some("end") => {
                notes.push(("text-align", "end".into(), "right".into()));
                TextAlign::Right
            }
            Some("start") => {
                notes.push(("text-align", "start".into(), "left".into()));
                TextAlign::Left
            }
            Some("justify") => {
                notes.push(("text-align", "justify".into(), "left".into()));
                TextAlign::Left
            }
            _ => TextAlign::Left,
        };
        let transform = get("text-transform").unwrap_or_default();
        if matches!(transform.as_str(), "uppercase" | "lowercase" | "capitalize") {
            notes.push((
                "text-transform",
                transform.clone(),
                "applied to the text".into(),
            ));
        }
        TextStyle {
            size: s.font_size,
            weight,
            color: s.color.clone(),
            align,
            ws: Ws::of(s),
            transform,
            notes,
        }
    }

    /// The text of a node: the pieces rendered and cased.
    pub fn text(&self, pieces: &[Piece]) -> String {
        transform(&render(pieces, self.ws), &self.transform)
    }

    /// Say what was approximated in reading this style.
    pub fn report(&self, n: &mut Notes) {
        for (p, from, to) in &self.notes {
            n.change(p, from, to);
        }
    }
}
