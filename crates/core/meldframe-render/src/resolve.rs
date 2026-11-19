use meldframe_protocol::{Bind, GradientKind, ImageFit, Node, NodeKind, Value, Variables};
use meldframe_variables::{ModeChoice, resolve as resolve_variable};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// What a box is drawn as. An ellipse is a rounded rectangle and a shader its plain box: the canvas
/// has no better drawing for them yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Paint {
    Frame,
    Rect,
    Text,
}

/// A gradient over the fill; every stop colour is a valid hex colour.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaintedGradient {
    pub kind: GradientKind,
    pub angle: f64,
    pub stops: Vec<(f64, String)>,
}

/// An image over the fill and the gradient.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintedImage {
    pub asset_id: String,
    pub fit: ImageFit,
}

/// One box ready to paint. `x` and `y` are the offset inside the parent, as the node stores them. It
/// serialises as the JSON the canvas reads (camel case; a gradient is `{kind, angle, stops: [[pos,
/// colour]]}`, and an absent gradient or image is left out).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintedBox {
    pub id: String,
    pub parent: Option<String>,
    pub paint: Paint,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// `0xRRGGBB`; `None` for no fill.
    pub fill: Option<u32>,
    pub radius: f64,
    pub text: String,
    pub font_size: f64,
    pub name: String,
    pub clip: bool,
    pub opacity: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gradient: Option<PaintedGradient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<PaintedImage>,
}

/// The painted boxes of a file's nodes, back to front: a parent before its children, siblings in
/// the order of their position then id. A node whose parent is missing is a root, and nodes that
/// only point at each other are kept after the rest, so a bad file stays visible.
pub fn resolve(nodes: &[Node]) -> Vec<PaintedBox> {
    resolve_with(nodes, &Variables::default(), &ModeChoice::default())
}

/// Like [`resolve`] for a file with variables: a bound field paints the value its variable has in the modes
/// `choice` picks. A binding that cannot be resolved (a variable that is not there, a chain that does not end)
/// paints the default of its field, never an error, so a bad file stays visible. The choice is view state:
/// it changes what is painted and never the nodes.
pub fn resolve_with(nodes: &[Node], vars: &Variables, choice: &ModeChoice) -> Vec<PaintedBox> {
    let cx = Cx { vars, choice };
    let ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut kids: HashMap<Option<&str>, Vec<&Node>> = HashMap::new();
    for n in nodes {
        let key = n.parent_id.as_deref().filter(|p| ids.contains(p));
        kids.entry(key).or_default().push(n);
    }
    for list in kids.values_mut() {
        list.sort_by(|a, b| sibling_order(a, b));
    }
    let mut out = Vec::with_capacity(nodes.len());
    let mut seen = HashSet::new();
    visit(None, &kids, &mut seen, &mut out, &cx);
    for n in nodes {
        if seen.insert(n.id.as_str()) {
            out.push(paint(n, None, &cx));
            visit(Some(&n.id), &kids, &mut seen, &mut out, &cx);
        }
    }
    out
}

/// The variables and modes a bound field is resolved with.
struct Cx<'a> {
    vars: &'a Variables,
    choice: &'a ModeChoice,
}

impl Cx<'_> {
    /// The value of a variable in the chosen modes.
    fn value(&self, var: &str) -> Option<Value> {
        resolve_variable(self.vars, var, self.choice)
    }

    /// A number field: the literal, or the variable's number held to `lo..=hi`; `None` when a binding does not
    /// resolve to a finite number.
    fn number(&self, b: &Bind<f64>, lo: f64, hi: f64) -> Option<f64> {
        match b {
            Bind::Lit(v) => Some(*v),
            Bind::Var(r) => match self.value(&r.var) {
                Some(Value::Number(v)) if v.is_finite() => Some(v.clamp(lo, hi)),
                _ => None,
            },
        }
    }

    /// The fill: `0xRRGGBB`, or `None` for no fill. An unresolved binding paints the default fill.
    fn fill(&self, b: &Bind<String>) -> Option<u32> {
        match b {
            Bind::Lit(css) => parse_fill(css),
            Bind::Var(r) => match self.value(&r.var) {
                Some(Value::Color(css)) => parse_fill(&css),
                _ => parse_fill(DEFAULT_FILL),
            },
        }
    }
}

/// The fill of a node that has none of its own (`Node`'s default).
const DEFAULT_FILL: &str = "#3f3f46";

fn visit<'a>(
    parent: Option<&'a str>,
    kids: &HashMap<Option<&'a str>, Vec<&'a Node>>,
    seen: &mut HashSet<&'a str>,
    out: &mut Vec<PaintedBox>,
    cx: &Cx,
) {
    for n in kids.get(&parent).into_iter().flatten() {
        if seen.insert(n.id.as_str()) {
            out.push(paint(n, parent, cx));
            visit(Some(n.id.as_str()), kids, seen, out, cx);
        }
    }
}

/// Sibling order is (position, id); a node without a position keeps list order and comes before
/// every positioned node.
fn sibling_order(a: &Node, b: &Node) -> Ordering {
    a.pos.cmp(&b.pos).then_with(|| {
        if a.pos.is_empty() {
            Ordering::Equal
        } else {
            a.id.cmp(&b.id)
        }
    })
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa` as `0xRRGGBB` (the alpha is dropped); anything else is no fill.
fn parse_fill(css: &str) -> Option<u32> {
    let hex = css.strip_prefix('#')?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        3 => {
            let v = u32::from_str_radix(hex, 16).ok()?;
            let (r, g, b) = ((v >> 8) & 15, (v >> 4) & 15, v & 15);
            Some(((r * 17) << 16) | ((g * 17) << 8) | (b * 17))
        }
        6 => u32::from_str_radix(hex, 16).ok(),
        8 => u32::from_str_radix(&hex[..6], 16).ok(),
        _ => None,
    }
}

/// Any hex colour a stop may carry: `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`.
fn is_hex_colour(css: &str) -> bool {
    css.strip_prefix('#').is_some_and(|h| {
        matches!(h.len(), 3 | 4 | 6 | 8) && h.bytes().all(|b| b.is_ascii_hexdigit())
    })
}

fn paint(n: &Node, parent: Option<&str>, cx: &Cx) -> PaintedBox {
    let is_text = n.kind == NodeKind::Text;
    let gradient = n
        .gradient
        .as_ref()
        .filter(|g| !is_text && g.stops.iter().all(|s| is_hex_colour(&s.color)))
        .map(|g| PaintedGradient {
            kind: g.kind,
            angle: g.angle,
            stops: g.stops.iter().map(|s| (s.pos, s.color.clone())).collect(),
        });
    let image = n.image.as_ref().filter(|_| !is_text).map(|i| PaintedImage {
        asset_id: i.asset_id.clone(),
        fit: i.fit,
    });
    let radius = cx.number(&n.radius, 0.0, f64::MAX).unwrap_or(0.0);
    let opacity = cx.number(&n.opacity, 0.0, 1.0).unwrap_or(1.0);
    let (paint, radius) = match n.kind {
        NodeKind::Frame => (Paint::Frame, radius),
        NodeKind::Text => (Paint::Text, 0.0),
        NodeKind::Ellipse => (Paint::Rect, n.w.min(n.h) / 2.0),
        NodeKind::Rect | NodeKind::Shader => (Paint::Rect, radius),
    };
    PaintedBox {
        id: n.id.clone(),
        parent: parent.map(str::to_owned),
        paint,
        x: n.x,
        y: n.y,
        w: n.w,
        h: n.h,
        fill: cx.fill(&n.fill),
        radius,
        text: if is_text {
            n.text.clone()
        } else {
            String::new()
        },
        font_size: if is_text { n.font_size } else { 0.0 },
        name: n.name.clone(),
        clip: n.clip,
        // Like the canvas, an opacity counts only when it is a number below 1.
        opacity: if opacity < 1.0 { opacity } else { 1.0 },
        gradient,
        image,
    }
}
