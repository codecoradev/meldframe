//! The box of a node: its layout, gap, padding, margin, alignment, clipping and place (`docs/import-html.md` section
//! 3.4). Everything is read from the computed style; a value that is not read is refused on the node, never guessed.

use super::item::Notes;
use super::pre::display;
use crate::css::{Style, value::Unit};
use meldframe_protocol::{Align, Justify, LayoutMode};

/// How the children of a frame are laid out, as they see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flow {
    Row,
    Column,
    /// No layout: the children sit at their own place.
    Free,
}

/// What a frame gives its children: the flow, whether an auto cross size stretches, and its own size.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ctx {
    pub flow: Flow,
    pub stretch: bool,
    pub sw: meldframe_protocol::SizeMode,
    pub sh: meldframe_protocol::SizeMode,
    /// The size in pixels, when it is fixed.
    pub px: (Option<f64>, Option<f64>),
}

fn fmt(v: f64) -> String {
    format!("{v}px")
}

/// The most a coordinate may be (`meldframe_check::Limits::max_coord`), and a size (`max_size`).
const MAX_COORD: f64 = 1.0e6;
pub(crate) const MAX_SIZE: f64 = 100_000.0;

/// A length in pixels. `None` when the property is absent, or not a length or out of the bounds of a coordinate
/// (refused, with its value).
pub(crate) fn px(st: &Style, prop: &str, notes: &mut Notes) -> Option<f64> {
    let raw = st.get(prop)?.trim();
    match st.len(prop) {
        Some(l) if l.px.abs() > MAX_COORD => {
            notes.refuse(prop, raw, "refused_out_of_range");
            None
        }
        Some(l) => {
            if l.unit != Unit::Px {
                notes.approximate(prop, raw, &fmt(l.px));
            }
            Some(l.px)
        }
        None => {
            notes.refuse(prop, raw, "refused_value");
            None
        }
    }
}

/// A size in pixels: a length that is not negative and within the bounds of a size.
pub(crate) fn size_px(st: &Style, prop: &str, notes: &mut Notes) -> Option<f64> {
    let v = px(st, prop, notes)?;
    if v < 0.0 {
        notes.refuse(prop, st.get(prop).unwrap_or(""), "refused_value");
        None
    } else if v > MAX_SIZE {
        notes.refuse(prop, st.get(prop).unwrap_or(""), "refused_out_of_range");
        None
    } else {
        Some(v)
    }
}

/// The four sides of `prefix` (`padding` or `margin`), top first; a side that is absent is 0, one that is negative or
/// not a length is refused and 0.
pub(crate) fn sides(st: &Style, prefix: &str, notes: &mut Notes) -> [f64; 4] {
    ["top", "right", "bottom", "left"]
        .map(|s| size_px(st, &format!("{prefix}-{s}"), notes).unwrap_or(0.0))
}

/// The padding fields of a node: one value when the sides are equal, else all four sides.
pub(crate) fn set_padding(node: &mut meldframe_protocol::Node, p: [f64; 4]) {
    if p.iter().all(|v| *v == p[0]) {
        node.padding = p[0];
    } else {
        node.padding = 0.0;
        [node.pad_t, node.pad_r, node.pad_b, node.pad_l] = p.map(Some);
    }
}

/// What a container reads of its style.
pub(crate) struct Container {
    pub layout: LayoutMode,
    pub flow: Flow,
    pub stretch: bool,
    pub gap: f64,
    pub align: Align,
    pub justify: Justify,
}

/// `flex-start`, `start`, `center`, `flex-end`, `end` with or without `safe`; `None` for another word.
fn position(v: &str) -> Option<Align> {
    let v = v.trim().to_ascii_lowercase();
    let v = v
        .strip_prefix("safe ")
        .or(v.strip_prefix("unsafe "))
        .unwrap_or(&v);
    match v.trim() {
        "flex-start" | "start" | "self-start" => Some(Align::Start),
        "center" => Some(Align::Center),
        "flex-end" | "end" | "self-end" => Some(Align::End),
        _ => None,
    }
}

/// The layout of a container. `free`: it has no layout of its own and its children sit at their own place.
pub(crate) fn container(st: &Style, free: bool, notes: &mut Notes) -> Container {
    let d = display(st);
    let flex = matches!(d.as_str(), "flex" | "inline-flex");
    let mut c = Container {
        layout: LayoutMode::Column,
        flow: Flow::Column,
        stretch: true,
        gap: 0.0,
        align: Align::Start,
        justify: Justify::Start,
    };
    if matches!(d.as_str(), "grid" | "inline-grid") {
        notes.refuse("display", &d, "refused_grid");
    }
    if d == "table-row" {
        (c.layout, c.flow) = (LayoutMode::Row, Flow::Row);
    }
    if free {
        (c.layout, c.flow) = (LayoutMode::None, Flow::Free);
    }
    if !flex {
        return c;
    }
    if let Some(v) = st
        .get("flex-direction")
        .map(|v| v.trim().to_ascii_lowercase())
    {
        let column = v.starts_with("column");
        if v.ends_with("reverse") {
            notes.refuse("flex-direction", &v, "refused_reverse");
        } else if !matches!(v.as_str(), "row" | "column") {
            notes.refuse("flex-direction", &v, "refused_value");
        }
        if column {
            (c.layout, c.flow) = (LayoutMode::Column, Flow::Column);
        } else {
            (c.layout, c.flow) = (LayoutMode::Row, Flow::Row);
        }
    } else {
        (c.layout, c.flow) = (LayoutMode::Row, Flow::Row);
    }
    c.stretch = false;
    match st.get("align-items").map(|v| v.trim().to_ascii_lowercase()) {
        None => c.stretch = true,
        Some(v) if matches!(v.as_str(), "stretch" | "normal") => c.stretch = true,
        Some(v) => match position(&v) {
            Some(a) => c.align = a,
            None => notes.refuse("align-items", &v, "refused_value"),
        },
    }
    if let Some(v) = st.get("justify-content") {
        let l = v.trim().to_ascii_lowercase();
        match (l.as_str(), position(&l)) {
            ("space-between", _) => c.justify = Justify::Between,
            ("normal", _) | (_, Some(Align::Start)) => {}
            (_, Some(Align::Center)) => c.justify = Justify::Center,
            (_, Some(Align::End)) => c.justify = Justify::End,
            _ => notes.refuse("justify-content", &l, "refused_value"),
        }
    }
    c.gap = gap(st, c.flow == Flow::Row, notes);
    c
}

/// The `align-self` of an element, when it is not one of the two words the mapping reads: listed once.
pub(crate) fn align_self(st: &Style, notes: &mut Notes) {
    if let Some(v) = st.get("align-self").map(|v| v.trim().to_ascii_lowercase())
        && !matches!(v.as_str(), "auto" | "normal" | "stretch")
    {
        notes.refuse("align-self", &v, "refused_value");
    }
}

/// The gap of the main axis. The model has one, so a different gap on the other axis is lost (listed).
fn gap(st: &Style, row: bool, notes: &mut Notes) -> f64 {
    let (main, cross) = if row {
        ("column-gap", "row-gap")
    } else {
        ("row-gap", "column-gap")
    };
    let num = |p: &str, notes: &mut Notes| match st.get(p).map(str::trim) {
        Some("normal") | None => None,
        Some(_) => size_px(st, p, notes),
    };
    let m = num(main, notes);
    match (m, num(cross, notes)) {
        (Some(m), Some(c)) if m != c => {
            notes.approximate("gap", &format!("{}px {}px", fmt_n(c), fmt_n(m)), &fmt(m))
        }
        (None, Some(c)) if c != 0.0 => {
            notes.refuse(cross, st.get(cross).unwrap_or(""), "refused_not_modelled")
        }
        _ => {}
    }
    m.unwrap_or(0.0)
}

fn fmt_n(v: f64) -> String {
    format!("{v}")
}

/// Does the element clip? `hidden`, `clip`, `auto` and `scroll` do (the last two approximated: nothing scrolls).
pub(crate) fn clip(st: &Style, notes: &mut Notes) -> bool {
    let word = |p: &str| st.get(p).map(|v| v.trim().to_ascii_lowercase());
    let (x, y) = (word("overflow-x"), word("overflow-y"));
    let clips =
        |v: &Option<String>| matches!(v.as_deref(), Some("hidden" | "clip" | "auto" | "scroll"));
    let on = clips(&x) || clips(&y);
    if on {
        for v in [&x, &y].into_iter().flatten() {
            if matches!(v.as_str(), "auto" | "scroll") {
                notes.approximate("overflow", v, "clip");
                break;
            }
        }
    }
    on
}

/// The place of a node: the pixels of `left` and `top` of an absolute box, and whether it leaves the flow of a layout
/// frame. A `right` or a `bottom`, an offset of a relative box, and a fixed or sticky position are refused.
pub(crate) fn place(st: &Style, ctx: &Ctx, notes: &mut Notes) -> (f64, f64, bool) {
    let pos = st.get("position").map_or("static", str::trim);
    let offset = |p: &str, notes: &mut Notes| match st.get(p).map(str::trim) {
        Some("auto") | None => None,
        Some(_) => px(st, p, notes),
    };
    match pos {
        "absolute" => {
            let (x, y) = (offset("left", notes), offset("top", notes));
            for p in ["right", "bottom"] {
                if let Some(v) = st.get(p).filter(|v| v.trim() != "auto") {
                    notes.refuse(p, v, "refused_value");
                }
            }
            (x.unwrap_or(0.0), y.unwrap_or(0.0), ctx.flow != Flow::Free)
        }
        "relative" => {
            for p in ["left", "top", "right", "bottom"] {
                if let Some(v) = st
                    .get(p)
                    .filter(|v| v.trim() != "auto" && px(st, p, &mut Notes::default()) != Some(0.0))
                {
                    notes.refuse(p, v, "refused_value");
                }
            }
            (0.0, 0.0, false)
        }
        "static" => (0.0, 0.0, false),
        other => {
            notes.refuse("position", other, "refused_value");
            (0.0, 0.0, false)
        }
    }
}
