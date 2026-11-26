//! The fields of a node that the mapping decides: the blank node (every field written explicitly, section 3.1),
//! the place of a box, its margins and padding, its flow (layout, alignment, gap) and what it is made of.

use super::notes::Notes;
use super::size::{Axis, Parent, converted};
use crate::css::Style;
use crate::paint::Asset;
use meldframe_protocol::{
    Align, Bind, Border, Justify, LayoutMode, Node, NodeKind, ShaderPreset, SizeMode, TextAlign,
};

/// A node of the result before its id is known: the tree of the page, with what is said about each node.
#[derive(Debug)]
pub(super) struct Spec {
    pub node: Node,
    pub source: String,
    pub notes: Notes,
    /// The addresses to list: `image` or `background`.
    pub assets: Vec<(&'static str, Asset)>,
    pub kids: Vec<Spec>,
    /// The author named the element (`data-name`, `id` or a class), so the name is not a role.
    pub named: bool,
}

impl Spec {
    /// The number of nodes in this tree.
    pub fn count(&self) -> usize {
        1 + self.kids.iter().map(Spec::count).sum::<usize>()
    }
}

/// A node with every field written as CSS would have it: no fill, no clip, no padding, no gap. The model's own
/// defaults (a grey fill, clip on, padding 16, gap 8) are not CSS's and never reach a node of an import.
pub(super) fn blank(kind: NodeKind) -> Node {
    Node {
        id: String::new(),
        name: String::new(),
        kind,
        parent_id: None,
        pos: String::new(),
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
        sw: SizeMode::Fixed,
        sh: SizeMode::Fixed,
        fill: Bind::Lit("none".into()),
        fill2: "#22d3ee".into(),
        radius: Bind::Lit(0.0),
        layout: LayoutMode::None,
        gap: 0.0,
        padding: 0.0,
        pad_t: None,
        pad_r: None,
        pad_b: None,
        pad_l: None,
        align: Align::Start,
        justify: Justify::Start,
        clip: false,
        text: String::new(),
        font_size: 16.0,
        shader: ShaderPreset::Aurora,
        font_weight: 400,
        text_align: TextAlign::Left,
        border: Border::default(),
        shadows: Vec::new(),
        mar_t: 0.0,
        mar_r: 0.0,
        mar_b: 0.0,
        mar_l: 0.0,
        absolute: false,
        opacity: Bind::Lit(1.0),
        gradient: None,
        image: None,
    }
}

/// A side of a box in pixels: `top`, `right`, `bottom`, `left`. A value that is not a length in pixels, em or rem, or
/// is negative, is listed and is 0.
fn sides(s: &Style, base: &str, n: &mut Notes) -> [f64; 4] {
    let mut out = [0.0; 4];
    for (i, side) in ["top", "right", "bottom", "left"].iter().enumerate() {
        let prop = format!("{base}-{side}");
        let Some(raw) = s.get(&prop).map(str::trim) else {
            continue;
        };
        match s.len(&prop) {
            Some(l) if l.px >= 0.0 => {
                converted(n, &prop, raw, l);
                out[i] = l.px;
            }
            Some(_) => n.lose(&prop, raw, "refused_negative"),
            None => n.lose(&prop, raw, "refused_unreadable"),
        }
    }
    out
}

/// The padding of a box on each side, with what its CSS border takes (`room`).
pub(super) fn padding(s: &Style, n: &mut Notes, room: f64) -> [f64; 4] {
    sides(s, "padding", n).map(|p| p + room)
}

/// The padding of a box on each side as the author wrote it, without what a border takes.
pub(super) fn authored_padding(s: &Style, n: &mut Notes) -> [f64; 4] {
    sides(s, "padding", n)
}

/// Write padding on a node: one `padding` when the four sides are equal, else each side.
pub(super) fn set_padding(node: &mut Node, p: [f64; 4]) {
    if p.iter().all(|v| *v == p[0]) {
        node.padding = p[0];
    } else {
        node.padding = p[0];
        node.pad_t = Some(p[0]);
        node.pad_r = Some(p[1]);
        node.pad_b = Some(p[2]);
        node.pad_l = Some(p[3]);
    }
}

/// The margin of a child of a frame that lays out, on each side.
pub(super) fn set_margin(node: &mut Node, s: &Style, n: &mut Notes) {
    let m = sides(s, "margin", n);
    node.mar_t = m[0];
    node.mar_r = m[1];
    node.mar_b = m[2];
    node.mar_l = m[3];
}

/// Where a box sits.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Place {
    /// `position: absolute`.
    pub absolute: bool,
    pub x: f64,
    pub y: f64,
}

/// `position` and its offsets: an absolute box at `left` and `top`; `fixed`, `sticky` and the offsets of a `relative`
/// box are refused and the box stays in the flow. (`right` and `bottom` are refused by the declaration scan.)
pub(super) fn place(s: &Style, n: &mut Notes) -> Place {
    let pos = s.get("position").map(|v| v.trim().to_ascii_lowercase());
    match pos.as_deref() {
        Some("absolute") => {
            let at = |prop: &str, n: &mut Notes| -> f64 {
                let Some(raw) = s.get(prop).map(str::trim) else {
                    return 0.0;
                };
                match s.len(prop) {
                    Some(l) => {
                        converted(n, prop, raw, l);
                        l.px
                    }
                    None => {
                        n.lose(prop, raw, "refused_unreadable");
                        0.0
                    }
                }
            };
            Place {
                absolute: true,
                x: at("left", n),
                y: at("top", n),
            }
        }
        Some(v @ ("fixed" | "sticky")) => {
            n.lose("position", v, "refused_not_modelled");
            Place::default()
        }
        Some("relative") => {
            for prop in ["top", "left"] {
                if let Some(raw) = s.get(prop) {
                    n.lose(prop, raw, "refused_offset");
                }
            }
            Place::default()
        }
        _ => Place::default(),
    }
}

/// How a box lays out its children, from its `display` and what is in it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Flow {
    pub layout: LayoutMode,
    pub flex: bool,
    pub stretch: bool,
    pub cells: bool,
    pub align: Align,
    pub justify: Justify,
    pub gap: f64,
}

/// The `display` of an element, in lower case; empty when it has none.
pub(super) fn display(s: &Style) -> String {
    s.get("display")
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_default()
}

fn after_safe(v: &str) -> &str {
    v.strip_prefix("safe ")
        .or_else(|| v.strip_prefix("unsafe "))
        .unwrap_or(v)
        .trim()
}

/// The flow of a frame. `all_absolute`: it holds only absolute boxes, so it has no layout (it places them where they
/// say). `button`: a button centres its label.
pub(super) fn flow(s: &Style, n: &mut Notes, all_absolute: bool, button: bool) -> Flow {
    let d = display(s);
    let flex = d.contains("flex");
    let grid = d.contains("grid");
    let table_row = d == "table-row";
    let plain = Flow {
        layout: LayoutMode::Column,
        flex: false,
        stretch: true,
        cells: false,
        align: Align::Start,
        justify: Justify::Start,
        gap: 0.0,
    };
    if grid {
        n.change("display", &d, "layout column");
        return plain;
    }
    if table_row {
        return Flow {
            layout: LayoutMode::Row,
            cells: true,
            ..plain
        };
    }
    if !flex {
        if button {
            return Flow {
                layout: LayoutMode::Row,
                stretch: false,
                align: Align::Center,
                justify: Justify::Center,
                ..plain
            };
        }
        if all_absolute {
            return Flow {
                layout: LayoutMode::None,
                ..plain
            };
        }
        return plain;
    }
    let get = |p: &str| s.get(p).map(|v| v.trim().to_ascii_lowercase());
    let layout = match get("flex-direction").as_deref() {
        None | Some("row") => LayoutMode::Row,
        Some("column") => LayoutMode::Column,
        Some(v @ ("row-reverse" | "column-reverse")) => {
            n.lose("flex-direction", v, "refused_not_modelled");
            if v.starts_with("row") {
                LayoutMode::Row
            } else {
                LayoutMode::Column
            }
        }
        Some(v) => {
            n.lose("flex-direction", v, "refused_unreadable");
            LayoutMode::Row
        }
    };
    if let Some(v @ ("wrap" | "wrap-reverse")) = get("flex-wrap").as_deref() {
        n.lose("flex-wrap", v, "refused_not_modelled");
    }
    let (mut stretch, mut align) = (false, Align::Start);
    match get("align-items").as_deref().map(after_safe) {
        None | Some("normal" | "stretch") => stretch = true,
        Some("flex-start" | "start" | "self-start" | "left") => {}
        Some("center") => align = Align::Center,
        Some("flex-end" | "end" | "self-end" | "right") => align = Align::End,
        Some(v) => n.lose("align-items", v, "refused_not_modelled"),
    }
    let justify = match get("justify-content").as_deref().map(after_safe) {
        None | Some("normal" | "flex-start" | "start" | "left") => Justify::Start,
        Some("center") => Justify::Center,
        Some("flex-end" | "end" | "right") => Justify::End,
        Some("space-between") => Justify::Between,
        Some(v) => {
            n.lose("justify-content", v, "refused_not_modelled");
            Justify::Start
        }
    };
    let len = |p: &str, n: &mut Notes| {
        let raw = s.get(p)?.trim();
        let l = s.len(p).filter(|l| l.px >= 0.0)?;
        converted(n, p, raw, l);
        Some(l.px)
    };
    let (col, row) = (len("column-gap", n), len("row-gap", n));
    if let (Some(a), Some(b)) = (col, row)
        && (a - b).abs() > f64::EPSILON
    {
        n.change("gap", &format!("{b}px {a}px"), "the gap of the main axis");
    }
    let gap = match layout {
        LayoutMode::Row => col,
        _ => row,
    }
    .unwrap_or(0.0);
    Flow {
        layout: if all_absolute && !flex {
            LayoutMode::None
        } else {
            layout
        },
        flex: true,
        stretch,
        cells: false,
        align,
        justify,
        gap,
    }
}

/// Write the size of a node: modes, and the pixels where the axis is fixed (grown by `grow` when the box is
/// `content-box`: the model's size is the border box).
pub(super) fn set_size(node: &mut Node, aw: Axis, ah: Axis, grow: (f64, f64)) {
    node.sw = aw.mode;
    node.sh = ah.mode;
    node.w = aw.px.map_or(0.0, |p| p + grow.0);
    node.h = ah.px.map_or(0.0, |p| p + grow.1);
}

/// Does the box size itself by `content-box` (the browser's default), so that padding and border are added to
/// its width and height?
pub(super) fn content_box(s: &Style) -> bool {
    !s.get("box-sizing")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("border-box"))
}

/// The fixed size of a box in pixels, to hand to its children for `100%`.
pub(super) fn fixed(a: Axis, grow: f64) -> Option<f64> {
    (a.mode == SizeMode::Fixed).then(|| a.px.unwrap_or(0.0) + grow)
}

/// A node that stands for something the importer does not carry: a rect of the size given, painting nothing.
pub(super) fn visible(node: &mut Node, s: &Style, n: &mut Notes) {
    if let Some(v) = s.get("visibility").map(|v| v.trim().to_ascii_lowercase())
        && matches!(v.as_str(), "hidden" | "collapse")
    {
        n.change("visibility", &v, "opacity 0");
        node.opacity = Bind::Lit(0.0);
    }
}

/// A parent for the children of a box.
pub(super) fn parent_of(f: &Flow, aw: Axis, ah: Axis, w: Option<f64>, h: Option<f64>) -> Parent {
    Parent {
        layout: f.layout,
        sw: aw.mode,
        sh: ah.mode,
        stretch: f.stretch,
        flex: f.flex,
        cells: f.cells,
        w,
        h,
    }
}
