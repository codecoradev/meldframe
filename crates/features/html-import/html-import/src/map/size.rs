//! The size of a node on one axis: fixed, hug or fill (`docs/import-html.md` section 3.4, the width and height rows).
//! A size is read from `width` or `height`, then from what the parent does with an auto size: a column or a block
//! stretches it across, a row lets it hug along, `flex-grow` fills it along, and a parent that hugs on that axis makes
//! a fill hug (rule G1 of the fidelity contract: a fill under a hug parent could not be exported).

use super::item::Notes;
use super::layout::{Ctx, Flow, size_px};
use super::pre::display;
use crate::css::Style;
use meldframe_protocol::SizeMode;

/// What one axis of a node is.
pub(crate) struct Axis {
    pub mode: SizeMode,
    /// The pixels of a fixed size, as the stylesheet gave them (not yet grown by padding).
    pub px: Option<f64>,
}

/// What an element is for sizing: whether it is a text, and the axis.
#[derive(Clone, Copy)]
pub(crate) struct Subject {
    pub text: bool,
    pub horizontal: bool,
    /// It cannot wrap: `white-space` of `nowrap` or `pre`.
    pub nowrap: bool,
    pub absolute: bool,
    /// The text has the pin of the export on it: `width: auto` with a `min-width` in pixels (see [`limits`]).
    pub pinned: bool,
}

fn is_zero(st: &Style, prop: &str) -> bool {
    st.len(prop).is_some_and(|l| l.px == 0.0)
}

/// `min-*` and `max-*` are not modelled: refused, except the initial values and the pin the export writes on a text
/// (`width: auto` with a `min-width`, rule G4), which says the text hugs: the importer measures it itself. Once for
/// each element: the result is whether the pin is there.
pub(crate) fn limits(st: &Style, text: bool, notes: &mut Notes) -> bool {
    let auto_width = st
        .get("width")
        .is_none_or(|w| w.trim().eq_ignore_ascii_case("auto"));
    let mut pin = false;
    for p in ["min-width", "min-height", "max-width", "max-height"] {
        let Some(v) = st.get(p).map(str::trim) else {
            continue;
        };
        if matches!(v, "auto" | "none" | "normal") || is_zero(st, p) {
            continue;
        }
        if p == "min-width" && text && auto_width {
            pin = true;
        } else {
            notes.refuse(p, v, "refused_not_modelled");
        }
    }
    pin
}

/// A number of `flex-grow`, `0` when absent or not a number.
fn grow(st: &Style) -> f64 {
    st.get("flex-grow")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v >= 0.0)
        .unwrap_or(0.0)
}

/// Resolve one axis. `st` is the style of the element, `ctx` what its parent gives it.
pub(crate) fn axis(st: &Style, s: Subject, ctx: &Ctx, notes: &mut Notes) -> Axis {
    let prop = if s.horizontal { "width" } else { "height" };
    let own = st
        .get(prop)
        .map(str::trim)
        .filter(|v| !v.eq_ignore_ascii_case("auto"));
    let (parent_mode, parent_px) = if s.horizontal {
        (ctx.sw, ctx.px.0)
    } else {
        (ctx.sh, ctx.px.1)
    };
    let parent_hugs = parent_mode == SizeMode::Hug;
    let in_layout = ctx.flow != Flow::Free && !s.absolute;
    let main = in_layout && ((ctx.flow == Flow::Row) == s.horizontal);
    let g = if main { grow(st) } else { 0.0 };
    let cell = display(st) == "table-cell" && main;
    let grows = g > 0.0 || cell;
    if g > 0.0 && g != 1.0 {
        notes.approximate("flex-grow", &g.to_string(), "fill");
    }
    let pinned = s.pinned && s.horizontal && own.is_none();
    let fixed = |v: f64| Axis {
        mode: SizeMode::Fixed,
        px: Some(v),
    };
    let mode = |mode| Axis { mode, px: None };
    if let Some(v) = own {
        let l = v.to_ascii_lowercase();
        return match l.as_str() {
            "fit-content" | "max-content" => mode(SizeMode::Hug),
            "min-content" => {
                notes.approximate(prop, &l, "hug");
                mode(SizeMode::Hug)
            }
            "100%" if in_layout && !parent_hugs => mode(SizeMode::Fill),
            "100%" if parent_px.is_some() => fixed(parent_px.unwrap_or(0.0)),
            _ if l.ends_with('%') || l.contains('(') => {
                notes.refuse(prop, &l, "refused_value");
                auto(
                    st,
                    s,
                    ctx,
                    parent_hugs,
                    grows,
                    in_layout,
                    main,
                    pinned,
                    notes,
                )
            }
            _ => match size_px(st, prop, notes) {
                Some(_) if grows && in_layout && !parent_hugs => mode(SizeMode::Fill),
                Some(v) => fixed(v),
                None => {
                    notes.refuse(prop, &l, "refused_value");
                    auto(
                        st,
                        s,
                        ctx,
                        parent_hugs,
                        grows,
                        in_layout,
                        main,
                        pinned,
                        notes,
                    )
                }
            },
        };
    }
    auto(
        st,
        s,
        ctx,
        parent_hugs,
        grows,
        in_layout,
        main,
        pinned,
        notes,
    )
}

/// An auto size.
#[allow(clippy::too_many_arguments)]
fn auto(
    st: &Style,
    s: Subject,
    ctx: &Ctx,
    parent_hugs: bool,
    grows: bool,
    in_layout: bool,
    main: bool,
    pinned: bool,
    notes: &mut Notes,
) -> Axis {
    let mode = |mode| Axis { mode, px: None };
    if !in_layout || pinned {
        return mode(SizeMode::Hug);
    }
    if main {
        if grows && !parent_hugs {
            return mode(SizeMode::Fill);
        }
        let basis = st
            .get("flex-basis")
            .map(str::trim)
            .filter(|b| *b != "auto" && *b != "0%");
        if let Some(b) = basis
            && !grows
        {
            return match size_px(st, "flex-basis", notes) {
                Some(v) => Axis {
                    mode: SizeMode::Fixed,
                    px: Some(v),
                },
                None => {
                    notes.refuse("flex-basis", b, "refused_value");
                    mode(SizeMode::Hug)
                }
            };
        }
        return mode(SizeMode::Hug);
    }
    // The cross axis: it stretches unless the parent aligns, or the element is a text that is tall as its lines.
    let stretch = match st.get("align-self").map(|v| v.trim().to_ascii_lowercase()) {
        None => ctx.stretch,
        Some(v) if matches!(v.as_str(), "auto" | "normal") => ctx.stretch,
        Some(v) => v == "stretch",
    };
    let tall_as_lines = s.text && !s.horizontal;
    let one_line = s.text && s.nowrap;
    if stretch && !parent_hugs && !tall_as_lines && !one_line {
        mode(SizeMode::Fill)
    } else {
        mode(SizeMode::Hug)
    }
}
