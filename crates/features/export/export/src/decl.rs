//! One node to its declarations: what the contract (`docs/fidelity-contract.md`, section 4) maps each field to.
//!
//! Geometry comes from the node and the layout core (text is pinned by what the measurer says, rule G4; a fill
//! under a hug parent is a fixed size, rule G1; `safe` alignment, rule G2). Paint comes from the painted box of the
//! render step, except where the field is bound: a bound fill, radius or opacity is `var(--...)`.

use crate::names::{asset_id_ok, css_escape, escape, hex_alpha, hex_colour, num, px};
use crate::report::Report;
use meldframe_layout::{Rect, TextMeasurer};
use meldframe_protocol::{
    Align, Bind, BorderPosition, ImageFit, Justify, LayoutMode, Node, NodeKind, SizeMode, TextAlign,
};
use meldframe_render::{Paint, PaintedBox, PaintedGradient};
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
    /// A text whose box the measurer decides: a hug width (`min-width` is pinned from it) or a hug height (the
    /// wrapped height). A fill height is set by the layout (flex or stretch) and is not counted. `line-height` comes
    /// from the measurer for every text, so this is not all of them.
    pub measured: bool,
}

/// What the styles of one export read.
pub(crate) struct Cx<'a> {
    pub names: &'a HashMap<String, String>,
    pub rects: &'a HashMap<String, Rect>,
    pub measurer: &'a dyn TextMeasurer,
    pub asset_base: &'a str,
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

/// The color of a bound or literal fill: `var(--x)` when the variable is in the export, else what was painted.
fn colour(
    cx: &Cx,
    bind: &Bind<String>,
    painted: Option<(u32, f64)>,
    id: &str,
    rep: &mut Report,
) -> Option<String> {
    if let Bind::Var(r) = bind {
        if let Some(name) = cx.names.get(&r.var) {
            return Some(format!("var({name})"));
        }
        rep.warn(
            "unresolved_binding",
            Some(id),
            format!(
                "fill is bound to {}, which does not resolve; exported as the default fill",
                r.var
            ),
        );
    }
    painted.map(|(rgb, alpha)| hex_alpha(rgb, alpha))
}

/// A number that is a length: `calc(var(--x) * 1px)` when bound, else the painted value in px.
fn length(
    cx: &Cx,
    bind: &Bind<f64>,
    painted: f64,
    field: &str,
    id: &str,
    rep: &mut Report,
) -> String {
    if let Bind::Var(r) = bind {
        if let Some(name) = cx.names.get(&r.var) {
            return format!("calc(var({name}) * 1px)");
        }
        rep.warn(
            "unresolved_binding",
            Some(id),
            format!(
                "{field} is bound to {}, which does not resolve; exported as the default",
                r.var
            ),
        );
    }
    px(painted)
}

fn gradient_css(g: &PaintedGradient) -> String {
    let stops: Vec<String> = g
        .stops
        .iter()
        .map(|(pos, c)| format!("{c} {}%", num((pos * 100.0 * 1e4).round() / 1e4)))
        .collect();
    match g.kind {
        meldframe_protocol::GradientKind::Linear => {
            format!("linear-gradient({}deg,{})", num(g.angle), stops.join(","))
        }
        meldframe_protocol::GradientKind::Radial => {
            format!(
                "radial-gradient(circle farthest-corner,{})",
                stops.join(",")
            )
        }
    }
}

/// A colour field that a stylesheet may carry: the hex text as it is, else `transparent` and an `invalid_colour`
/// finding (the text is never copied: a stylesheet takes hex colours only).
fn safe_colour(text: &str, field: &str, id: &str, rep: &mut Report) -> String {
    match hex_colour(text) {
        Some(c) => c.to_owned(),
        None => {
            rep.unsupported(
                "invalid_colour",
                id,
                &format!("{field} is not a hex colour; exported as transparent"),
            );
            "transparent".into()
        }
    }
}

/// The declarations of one node.
pub(crate) fn style(cx: &Cx, n: &Node, b: &PaintedBox, place: &Place, rep: &mut Report) -> Style {
    let mut s = Style::default();
    let text = n.kind == NodeKind::Text;
    let parent_layout = place.parent.filter(|p| is_layout(p));
    let in_flow = parent_layout.is_some() && !n.absolute;
    let at_own_place = place.root.is_some() || (!in_flow && place.parent.is_some());
    let border = !text && n.border.width > 0.0;
    let rect = cx.rects.get(n.id.as_str()).copied().unwrap_or(Rect {
        x: n.x,
        y: n.y,
        w: n.w,
        h: n.h,
    });

    // Position: an absolute box at its own x and y, or relative when it positions a child or its border.
    if at_own_place {
        let (x, y) = place.root.unwrap_or((n.x, n.y));
        s.main
            .extend([d("position", "absolute"), d("left", px(x)), d("top", px(y))]);
    } else if place.holds_absolute || border {
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
        if border && n.border.position != BorderPosition::Inside {
            rep.unsupported(
                "border_clipped",
                &n.id,
                "a frame that clips also clips a border drawn outside or on its edge",
            );
        }
    }

    // Text.
    if text {
        let mut one = n.clone();
        one.text = "M".into();
        let line = cx.measurer.measure(&one, None).1;
        s.measured = aw == Axis::Hug || ah == Axis::Hug;
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
        let c = colour(cx, &n.fill, b.painted_fill(), &n.id, rep)
            .unwrap_or_else(|| "transparent".into());
        s.main.push(d("color", c));
        for (field, on) in [
            ("border", n.border.width > 0.0),
            ("gradient", n.gradient.is_some()),
            ("image", n.image.is_some()),
            ("shadows", !n.shadows.is_empty()),
        ] {
            if on {
                rep.unsupported(
                    "on_text",
                    &n.id,
                    &format!("a text node paints no {field}; it was left out"),
                );
            }
        }
        if text_outside_contract(&n.text) {
            rep.warn("text_outside_contract", Some(&n.id), "the text has a tab, a URL or a script (CJK) that the browser breaks differently from the canvas".into());
        }
    } else {
        paint(cx, n, b, &mut s, rep);
    }
    match (
        n.opacity.var_id(),
        n.opacity.var_id().and_then(|v| cx.names.get(v)),
    ) {
        (_, Some(name)) => s.main.push(d("opacity", format!("var({name})"))),
        (Some(v), None) => rep.warn(
            "unresolved_binding",
            Some(&n.id),
            format!("opacity is bound to {v}, which does not resolve; exported as the default"),
        ),
        (None, None) if b.opacity < 1.0 => s.main.push(d("opacity", num(b.opacity))),
        (None, None) => {}
    }
    if border {
        after(cx, n, b, rect, &mut s, rep);
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

/// Fill, gradient, image, radius, shadows and opacity of a box that is not text.
fn paint(cx: &Cx, n: &Node, b: &PaintedBox, s: &mut Style, rep: &mut Report) {
    let id = &n.id;
    let fill = colour(cx, &n.fill, b.painted_fill(), id, rep);
    let mut layers: Vec<(String, &str)> = Vec::new();
    let mut image = b.image.as_ref();
    if let Some(img) = image
        && !asset_id_ok(&img.asset_id)
    {
        rep.unsupported(
            "invalid_asset",
            id,
            "the image is not an asset id (64 hex digits); it was left out",
        );
        image = None;
    }
    if let Some(img) = image {
        rep.asset(&img.asset_id);
        let size = match img.fit {
            ImageFit::Cover => "cover",
            ImageFit::Contain => "contain",
            ImageFit::Fill => "100% 100%",
        };
        let base = css_escape(cx.asset_base, "-_./:");
        layers.push((format!("url({base}{})", img.asset_id), size));
    }
    if let Some(g) = &b.gradient {
        layers.push((gradient_css(g), "100% 100%"));
    }
    if n.kind == NodeKind::Shader {
        let c1 = fill.clone().unwrap_or_else(|| "transparent".into());
        let c2 = safe_colour(&n.fill2, "fill2", id, rep);
        layers.push((format!("linear-gradient(135deg,{c1},{c2})"), "100% 100%"));
        rep.unsupported("shader_approximated", id, "a shader has no CSS form: it is exported as a gradient from fill to fill2, whatever its preset");
    } else if let Some(c) = fill {
        s.main.push(d("background-color", c));
    }
    if !layers.is_empty() {
        let urls: Vec<&str> = layers.iter().map(|l| l.0.as_str()).collect();
        s.main.push(d("background-image", urls.join(",")));
        if image.is_some() {
            let size: Vec<&str> = layers.iter().map(|l| l.1).collect();
            s.main.extend([
                d("background-size", size.join(",")),
                d("background-position", "center"),
                d("background-repeat", "no-repeat"),
            ]);
        }
    }
    if n.kind == NodeKind::Ellipse {
        s.main.push(d("border-radius", "50%"));
    } else if b.radius > 0.0 || matches!(n.radius, Bind::Var(_)) {
        let v = length(cx, &n.radius, b.radius, "radius", id, rep);
        if v != "0px" {
            s.main.push(d("border-radius", v));
        }
    }
    if !n.shadows.is_empty() {
        // The model draws the first shadow at the bottom; CSS draws the first at the top.
        let list: Vec<String> = n
            .shadows
            .iter()
            .rev()
            .map(|w| {
                format!(
                    "{} {} {} {} {}",
                    px(w.x),
                    px(w.y),
                    px(w.blur),
                    px(w.spread),
                    safe_colour(&w.color, "shadows.color", id, rep)
                )
            })
            .collect();
        s.main.push(d("box-shadow", list.join(",")));
    }
}

/// The border: paint only, as `::after` on the box, so that it takes no room (contract, section 4.2).
fn after(cx: &Cx, n: &Node, b: &PaintedBox, rect: Rect, s: &mut Style, rep: &mut Report) {
    let w = n.border.width;
    let (inset, grow) = match n.border.position {
        BorderPosition::Inside => (0.0, 0.0),
        BorderPosition::Center => (-w / 2.0, w / 2.0),
        BorderPosition::Outside => (-w, w),
    };
    let radius = if n.kind == NodeKind::Ellipse {
        Some("50%".to_owned())
    } else if let Some(name) = n.radius.var_id().and_then(|v| cx.names.get(v)) {
        let (hw, hh) = (px(rect.w / 2.0), px(rect.h / 2.0));
        Some(format!(
            "calc(min(var({name}) * 1px, {hw}, {hh}) + {})",
            px(grow)
        ))
    } else if b.radius > 0.0 {
        Some(px(b.radius.min(rect.w / 2.0).min(rect.h / 2.0) + grow))
    } else {
        None
    };
    s.after.extend([
        d("content", "\"\""),
        d("position", "absolute"),
        d("pointer-events", "none"),
        d("inset", px(inset)),
    ]);
    let colour = safe_colour(&n.border.color, "border.color", &n.id, rep);
    s.after
        .push(d("border", format!("{} solid {colour}", px(w))));
    if let Some(r) = radius {
        s.after.push(d("border-radius", r));
    }
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
