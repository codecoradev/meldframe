//! One node to its declarations: what the contract (`docs/fidelity-contract.md`, section 4) maps each field to.
//!
//! Geometry comes from the node and the layout core (text is pinned by what the measurer says, rule G4; a fill
//! under a hug parent is a fixed size, rule G1; `safe` alignment, rule G2). Paint comes from the painted box of the
//! render step, except where the field is bound: a bound fill, radius or opacity is `var(--...)`.

use crate::names::{escape, px};
use crate::report::Report;
use meldframe_layout::{Rect, TextMeasurer};
use meldframe_protocol::{Align, Justify, LayoutMode, Node, NodeKind, SizeMode, TextAlign};
use meldframe_render::{Paint, PaintedBox};
use std::collections::HashMap;

/// One CSS declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decl {
    pub prop: &'static str,
    pub value: String,
}

/// The declarations of a node: on the element, and on its `::after` (the border).
#[derive(Debug, Default)]
pub(crate) struct Style {
    pub main: Vec<Decl>,
    pub after: Vec<Decl>,
}

/// What the styles of one export read.
pub(crate) struct Cx<'a> {
    pub rects: &'a HashMap<String, Rect>,
    pub measurer: &'a dyn TextMeasurer,
}

/// The place of a node in its parent, worked out by the caller.
pub(crate) struct Place<'a> {
    pub parent: Option<&'a Node>,
    /// The exported frame, with the place it takes: its own x and y when it is a top-level frame, else the origin.
    pub root: Option<(f64, f64)>,
    /// Some child sits at its own x and y, so this node is the box that positions it.
    pub holds_absolute: bool,
    /// The size of a fill on the main axis when its siblings need the core's size (unequal padding), else `None`.
    pub basis: Option<f64>,
}

fn d(prop: &'static str, value: impl Into<String>) -> Decl {
    Decl {
        prop,
        value: value.into(),
    }
}

/// The four sides as one CSS value (`a`, or `t r b l`); `None` when all are zero.
fn sides(t: f64, r: f64, b: f64, l: f64) -> Option<String> {
    if (t, r, b, l) == (0.0, 0.0, 0.0, 0.0) {
        None
    } else if t == r && r == b && b == l {
        Some(px(t))
    } else {
        Some(format!("{} {} {} {}", px(t), px(r), px(b), px(l)))
    }
}

fn is_layout(n: &Node) -> bool {
    n.kind == NodeKind::Frame && n.layout != LayoutMode::None
}

#[derive(PartialEq)]
enum Axis {
    Fixed,
    Hug,
    FillMain,
    FillCross,
}

/// How a size is exported: rule G1 makes a fill that has no layout parent to share from, or whose parent hugs on
/// that axis, a fixed size; a hug that is not a layout frame or a text is fixed too, as in the core.
fn axis(n: &Node, mode: SizeMode, place: &Place, horizontal: bool) -> Axis {
    match mode {
        SizeMode::Fixed => Axis::Fixed,
        SizeMode::Hug if n.kind == NodeKind::Text || is_layout(n) => Axis::Hug,
        SizeMode::Hug => Axis::Fixed,
        SizeMode::Fill => match place.parent.filter(|p| is_layout(p) && !n.absolute) {
            Some(p) if (if horizontal { p.sw } else { p.sh }) != SizeMode::Hug => {
                if (p.layout == LayoutMode::Row) == horizontal {
                    Axis::FillMain
                } else {
                    Axis::FillCross
                }
            }
            _ => Axis::Fixed,
        },
    }
}

/// The declarations of one node.
pub(crate) fn style(cx: &Cx, n: &Node, place: &Place, rep: &mut Report) -> Style {
    let mut s = Style::default();
    let text = n.kind == NodeKind::Text;
    let parent_layout = place.parent.filter(|p| is_layout(p));
    let in_flow = parent_layout.is_some() && !n.absolute;
    let at_own_place = place.root.is_some() || (!in_flow && place.parent.is_some());

    // Position: an absolute box at its own x and y, or relative when it positions a child or its border.
    if at_own_place {
        let (x, y) = place.root.unwrap_or((n.x, n.y));
        s.main
            .extend([d("position", "absolute"), d("left", px(x)), d("top", px(y))]);
    } else if place.holds_absolute {
        s.main.push(d("position", "relative"));
    }

    // Size.
    let (aw, ah) = (axis(n, n.sw, place, true), axis(n, n.sh, place, false));
    let row = parent_layout.is_some_and(|p| p.layout == LayoutMode::Row);
    let mut item = Vec::new();
    for (horizontal, a, size) in [(true, &aw, n.w), (false, &ah, n.h)] {
        let (prop, min) = if horizontal {
            ("width", "min-width")
        } else {
            ("height", "min-height")
        };
        match a {
            Axis::Fixed => {
                s.main.push(d(prop, px(size)));
                if in_flow && row == horizontal {
                    item.push(d("flex-shrink", "0"));
                }
            }
            Axis::Hug if text && horizontal => {
                let width = cx.measurer.measure(n, None).0.ceil();
                s.main.extend([d("width", "auto"), d(min, px(width))]);
            }
            Axis::Hug if horizontal && !text => s.main.push(d(prop, "fit-content")),
            Axis::Hug => s.main.push(d(prop, "auto")),
            Axis::FillMain => {
                let basis = place.basis.map_or("0%".to_owned(), px);
                item.extend([d("flex", format!("1 1 {basis}")), d(min, "0")]);
            }
            Axis::FillCross => item.push(d("align-self", "stretch")),
        }
    }
    s.main.append(&mut item);
    if in_flow && let Some(m) = sides(n.mar_t, n.mar_r, n.mar_b, n.mar_l) {
        s.main.push(d("margin", m));
    }

    // Layout container.
    if is_layout(n) {
        let dir = if n.layout == LayoutMode::Row {
            "row"
        } else {
            "column"
        };
        s.main
            .extend([d("display", "flex"), d("flex-direction", dir)]);
        if n.gap > 0.0 {
            s.main.push(d("gap", px(n.gap)));
        }
        let p = n.pad();
        if let Some(v) = sides(p.top, p.right, p.bottom, p.left) {
            s.main.push(d("padding", v));
        }
        let align = match n.align {
            Align::Start => "flex-start",
            Align::Center => "center",
            Align::End => "flex-end",
        };
        // Rule G2: `safe`, so that overflow clamps to the start edge as the core does.
        let justify = match n.justify {
            Justify::Start => "flex-start",
            Justify::Center => "safe center",
            Justify::End => "safe flex-end",
            Justify::Between => "space-between",
        };
        s.main
            .extend([d("align-items", align), d("justify-content", justify)]);
    }
    if n.kind == NodeKind::Frame && n.clip {
        s.main.push(d("overflow", "hidden"));
    }

    // Text.
    if text {
        let mut one = n.clone();
        one.text = "M".into();
        let line = cx.measurer.measure(&one, None).1;
        s.main.extend([
            d("font-size", px(n.font_size)),
            d("font-weight", n.font_weight.to_string()),
            d("line-height", px(line)),
        ]);
        s.main.push(d(
            "white-space",
            if aw == Axis::Hug { "pre" } else { "pre-wrap" },
        ));
        s.main.push(d("word-break", "keep-all"));
        match n.text_align {
            TextAlign::Left => {}
            TextAlign::Center => s.main.push(d("text-align", "center")),
            TextAlign::Right => s.main.push(d("text-align", "right")),
        }
        if text_outside_contract(&n.text) {
            rep.warn("text_outside_contract", Some(&n.id), "the text has a tab, a URL or a script (CJK) that the browser breaks differently from the canvas".into());
        }
    }
    s
}

/// A tab, a URL or CJK text: breaks the browser and the canvas place differently (contract, section 4.3).
fn text_outside_contract(t: &str) -> bool {
    t.contains('\t')
        || t.contains("://")
        || t.chars().any(|c| {
            ('\u{3000}'..='\u{9fff}').contains(&c) || ('\u{ac00}'..='\u{d7af}').contains(&c)
        })
}

/// The tag and the attributes that name a node, for the element.
pub(crate) fn tag(b: &PaintedBox) -> &'static str {
    if b.paint == Paint::Text { "p" } else { "div" }
}

/// `data-node` and `data-name` of an element.
pub(crate) fn attrs(n: &Node) -> String {
    let mut a = format!("data-node=\"{}\"", escape(&n.id));
    if !n.name.is_empty() {
        a.push_str(&format!(" data-name=\"{}\"", escape(&n.name)));
    }
    a
}
