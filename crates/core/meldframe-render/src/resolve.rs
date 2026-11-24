use meldframe_protocol::{
    Bind, BorderPosition, DEFAULT_FILL, DEFAULT_OPACITY, DEFAULT_RADIUS, GradientKind, ImageFit,
    Node, NodeKind, TextAlign, Value, Variables,
};
use meldframe_variables::{ModeChoice, Resolver};
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

/// A border over the box and its children: paint only, it takes no room. `width` is above 0 and the colour is a valid hex colour.
/// The canvas draws its ring on the shape of the box, so the ring of an ellipse follows the pill the fill is drawn as (see [`Paint`]); the export's
/// is a true ellipse, which differs when the ellipse is not a circle (contract, section 4.2).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintedBorder {
    pub width: f64,
    /// `0xRRGGBB`.
    pub color: u32,
    /// The alpha of the colour's own form, 0 to 1 (`#rrggbbaa` and `#rgba` carry one).
    pub alpha: f64,
    /// Which side of the box's edge the stroke lies on.
    pub position: BorderPosition,
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
    /// The alpha of the fill's own colour, 0 to 1 (`#rrggbbaa` and `#rgba` carry one; every other form is 1). It is not
    /// the node's `opacity`, which fades the whole node, children included. 1 when there is no fill.
    pub fill_alpha: f64,
    pub radius: f64,
    pub text: String,
    pub font_size: f64,
    /// 100..=900; 0 on a box that is not a text, like `font_size`.
    pub font_weight: u16,
    /// Where the lines sit in the box; left on a box that is not a text.
    pub text_align: TextAlign,
    pub name: String,
    pub clip: bool,
    pub opacity: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gradient: Option<PaintedGradient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<PaintedImage>,
    /// Drawn over the box and its children; absent for no border, and always on a text, which paints none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<PaintedBorder>,
}

impl PaintedBox {
    /// The fill with its own alpha, `(0xRRGGBB, 0..=1)`; `None` for no fill.
    pub fn painted_fill(&self) -> Option<(u32, f64)> {
        self.fill.map(|rgb| (rgb, self.fill_alpha))
    }
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
    let cx = Fields::new(vars, choice);
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

/// The one place a bound field becomes the value that is painted: the render step and the properties panel both
/// read through it, so the panel shows what the canvas paints. The variables are indexed once.
#[derive(Debug)]
pub struct Fields<'a> {
    resolver: Resolver<'a>,
}

impl<'a> Fields<'a> {
    pub fn new(vars: &'a Variables, choice: &'a ModeChoice) -> Self {
        Fields {
            resolver: Resolver::new(vars, choice),
        }
    }

    /// The value of a variable in the chosen modes.
    fn value(&self, var: &str) -> Option<Value> {
        self.resolver.value(var)
    }

    /// The corner radius that is painted: the literal or the resolved number, held to zero or more; a binding
    /// that does not resolve is the default radius.
    pub fn radius(&self, b: &Bind<f64>) -> f64 {
        self.number(b, 0.0, f64::MAX).unwrap_or(DEFAULT_RADIUS)
    }

    /// The opacity that is painted, between 0 and 1; a binding that does not resolve is the default.
    pub fn opacity(&self, b: &Bind<f64>) -> f64 {
        self.number(b, 0.0, 1.0).unwrap_or(DEFAULT_OPACITY)
    }

    /// The fill as the properties panel shows it: the literal as it is stored, or the colour the variable
    /// resolves to; a binding that does not resolve shows the default fill, as the canvas paints it.
    pub fn fill_text(&self, b: &Bind<String>) -> String {
        match b {
            Bind::Lit(css) => css.clone(),
            Bind::Var(r) => match self.value(&r.var) {
                Some(Value::Color(css)) => css,
                _ => DEFAULT_FILL.to_string(),
            },
        }
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
    pub fn fill(&self, b: &Bind<String>) -> Option<u32> {
        self.fill_rgba(b).map(|(rgb, _)| rgb)
    }

    /// The fill with its own alpha: `(0xRRGGBB, alpha)` with the alpha as the byte the colour carries (255 for a form with
    /// none), or `None` for no fill. An unresolved binding paints the default fill.
    pub fn fill_rgba(&self, b: &Bind<String>) -> Option<(u32, u8)> {
        match b {
            Bind::Lit(css) => parse_fill(css),
            Bind::Var(r) => match self.value(&r.var) {
                Some(Value::Color(css)) => parse_fill(&css),
                _ => parse_fill(DEFAULT_FILL),
            },
        }
    }
}

fn visit<'a>(
    parent: Option<&'a str>,
    kids: &HashMap<Option<&'a str>, Vec<&'a Node>>,
    seen: &mut HashSet<&'a str>,
    out: &mut Vec<PaintedBox>,
    cx: &Fields,
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

/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa` as `(0xRRGGBB, alpha byte)`; a form with no alpha has 255. Anything else is no
/// fill.
fn parse_fill(css: &str) -> Option<(u32, u8)> {
    let hex = css.strip_prefix('#')?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    let short = |v: u32| {
        // One digit each, doubled: `#f80` is `#ff8800`.
        let (r, g, b) = ((v >> 8) & 15, (v >> 4) & 15, v & 15);
        ((r * 17) << 16) | ((g * 17) << 8) | (b * 17)
    };
    match hex.len() {
        3 => Some((short(v), 255)),
        4 => Some((short(v >> 4), ((v & 15) * 17) as u8)),
        6 => Some((v, 255)),
        8 => Some((v >> 8, (v & 255) as u8)),
        _ => None,
    }
}

/// The alpha byte of a colour as the fraction the canvas and the export use, 0 to 1.
fn alpha_fraction(byte: u8) -> f64 {
    f64::from(byte) / 255.0
}

/// Any hex colour a stop may carry: `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`.
fn is_hex_colour(css: &str) -> bool {
    css.strip_prefix('#').is_some_and(|h| {
        matches!(h.len(), 3 | 4 | 6 | 8) && h.bytes().all(|b| b.is_ascii_hexdigit())
    })
}

fn paint(n: &Node, parent: Option<&str>, cx: &Fields) -> PaintedBox {
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
    let radius = cx.radius(&n.radius);
    let opacity = cx.opacity(&n.opacity);
    let fill = cx.fill_rgba(&n.fill);
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
        fill: fill.map(|(rgb, _)| rgb),
        fill_alpha: fill.map_or(1.0, |(_, a)| alpha_fraction(a)),
        radius,
        text: if is_text {
            n.text.clone()
        } else {
            String::new()
        },
        font_size: if is_text { n.font_size } else { 0.0 },
        font_weight: if is_text { n.font_weight } else { 0 },
        text_align: if is_text {
            n.text_align
        } else {
            TextAlign::default()
        },
        name: n.name.clone(),
        clip: n.clip,
        // Like the canvas, an opacity counts only when it is a number below 1.
        opacity: if opacity < 1.0 { opacity } else { 1.0 },
        gradient,
        image,
        border: if is_text { None } else { painted_border(n) },
    }
}

/// The border of a box that is not text: none for a width that is not above 0 or a colour that is not hex.
fn painted_border(n: &Node) -> Option<PaintedBorder> {
    let b = &n.border;
    if !(b.width > 0.0 && b.width.is_finite()) {
        return None;
    }
    let (color, alpha) = parse_fill(&b.color)?;
    Some(PaintedBorder {
        width: b.width,
        color,
        alpha: alpha_fraction(alpha),
        position: b.position,
    })
}
