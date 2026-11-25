//! The comparison of an imported file with the one that was exported (`docs/fidelity-contract.md`, the section on import).
//!
//! Both lists are in tree order (a parent before its children, siblings in their order), so node `i` of one is node `i`
//! of the other. Only the fields the export writes and the layout reads are compared, kind by kind, and a field is
//! compared by what it comes to and not by how it is stored: the losses of `docs/import-html.md` section 3.7 (the
//! ids, the `pos` strings, the stored size of a node that hugs or fills, a frame that is only a box coming back as a
//! rect) are not differences, and neither is what the core ignores (`hug` on a rect, `fill` where the parent
//! gives nothing to fill, `x` and `y` in a flow, `gap` and `padding` of a frame without layout), nor is a fill of
//! `transparent` that comes back as `none` (both paint nothing).

use meldframe_protocol::{LayoutMode, Node, NodeKind, SizeMode};
use serde_json::{Value, json};
use std::collections::HashMap;

/// A colour as it is written once: lower case, six digits, and no alpha when the alpha is 1.
fn colour(s: &str) -> String {
    let h = s.trim_start_matches('#').to_ascii_lowercase();
    let h: String = match h.len() {
        3 | 4 => h.chars().flat_map(|c| [c, c]).collect(),
        _ => h,
    };
    format!(
        "#{}",
        h.strip_suffix("ff").filter(|_| h.len() == 8).unwrap_or(&h)
    )
}

fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            (x.as_f64().unwrap_or(0.0) - y.as_f64().unwrap_or(1.0)).abs() < 1e-6
        }
        (Value::String(x), Value::String(y)) if x.starts_with('#') && y.starts_with('#') => {
            colour(x) == colour(y)
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, p)| y.get(k).is_some_and(|q| same(p, q)))
        }
        _ => a == b,
    }
}

struct Tree<'a> {
    by_id: HashMap<&'a str, &'a Node>,
    kids: HashMap<&'a str, usize>,
}

impl<'a> Tree<'a> {
    fn new(nodes: &'a [Node]) -> Self {
        let mut kids = HashMap::new();
        for n in nodes {
            if let Some(p) = &n.parent_id {
                *kids.entry(p.as_str()).or_insert(0) += 1;
            }
        }
        Tree {
            by_id: nodes.iter().map(|n| (n.id.as_str(), n)).collect(),
            kids,
        }
    }

    fn parent(&self, n: &Node) -> Option<&'a Node> {
        n.parent_id
            .as_deref()
            .and_then(|p| self.by_id.get(p).copied())
    }

    fn flows(&self, n: &Node) -> bool {
        self.parent(n)
            .is_some_and(|p| p.kind == NodeKind::Frame && p.layout != LayoutMode::None)
    }

    /// The size mode the layout core acts on: `hug` is for a text and for a frame with layout, and `fill` needs a flow
    /// parent that does not hug on that axis (rule G1).
    fn mode(&self, n: &Node, width: bool) -> SizeMode {
        let m = if width { n.sw } else { n.sh };
        let can_hug =
            n.kind == NodeKind::Text || (n.kind == NodeKind::Frame && n.layout != LayoutMode::None);
        match m {
            SizeMode::Hug if !can_hug => SizeMode::Fixed,
            SizeMode::Fill => match self.parent(n) {
                Some(p) if self.flows(n) && (if width { p.sw } else { p.sh }) != SizeMode::Hug => {
                    SizeMode::Fill
                }
                _ => SizeMode::Fixed,
            },
            m => m,
        }
    }

    /// Only a frame with nothing in it, no layout and a size in px is the same box as a rect (the export writes `div` for both).
    fn kind(&self, n: &Node) -> NodeKind {
        let boxy = n.kind == NodeKind::Frame
            && n.layout == LayoutMode::None
            && !self.kids.contains_key(n.id.as_str())
            && self.mode(n, true) == SizeMode::Fixed
            && self.mode(n, false) == SizeMode::Fixed;
        if boxy { NodeKind::Rect } else { n.kind }
    }
}

fn field(n: &Node, name: &str) -> Value {
    serde_json::to_value(n)
        .ok()
        .and_then(|v| v.get(name).cloned())
        .unwrap_or(Value::Null)
}

/// The fill, where `transparent` and `none` are the one fill that paints nothing (the import writes `none`).
fn fill(n: &Node) -> Value {
    match field(n, "fill") {
        Value::String(s) if s == "transparent" => json!("none"),
        v => v,
    }
}

fn border(n: &Node) -> Value {
    if n.border.width == 0.0 {
        json!({"width": 0})
    } else {
        field(n, "border")
    }
}

/// What differs between `original` and `imported`, as lines of text `<id> <field>: <was> became <is>`. Empty when
/// the import is the same file up to the losses the module comment lists.
pub fn fields(original: &[Node], imported: &[Node]) -> Vec<String> {
    if original.len() != imported.len() {
        return vec![format!(
            "{} nodes became {}",
            original.len(),
            imported.len()
        )];
    }
    let (to, ti) = (Tree::new(original), Tree::new(imported));
    let mut out = Vec::new();
    for (o, n) in original.iter().zip(imported) {
        let mut cmp = |name: &str, a: Value, b: Value| {
            if !same(&a, &b) {
                out.push(format!("{} {name}: {a} became {b}", o.id));
            }
        };
        let (ko, kn) = (to.kind(o), ti.kind(n));
        cmp("kind", json!(format!("{ko:?}")), json!(format!("{kn:?}")));
        cmp("name", json!(o.name), json!(n.name));
        let (swo, swn, sho, shn) = (
            to.mode(o, true),
            ti.mode(n, true),
            to.mode(o, false),
            ti.mode(n, false),
        );
        cmp("sw", json!(format!("{swo:?}")), json!(format!("{swn:?}")));
        cmp("sh", json!(format!("{sho:?}")), json!(format!("{shn:?}")));
        if swo == SizeMode::Fixed {
            cmp("w", json!(o.w), json!(n.w));
        }
        if sho == SizeMode::Fixed {
            cmp("h", json!(o.h), json!(n.h));
        }
        if o.parent_id.is_none() || !to.flows(o) {
            cmp("x", json!(o.x), json!(n.x));
            cmp("y", json!(o.y), json!(n.y));
        }
        if to.flows(o) {
            cmp(
                "margin",
                json!(format!("{:?}", o.margin())),
                json!(format!("{:?}", n.margin())),
            );
            cmp("absolute", json!(o.absolute), json!(n.absolute));
        }
        cmp("fill", fill(o), fill(n));
        cmp("opacity", field(o, "opacity"), field(n, "opacity"));
        if ko == NodeKind::Text {
            cmp("text", json!(o.text), json!(n.text));
            cmp("fontSize", json!(o.font_size), json!(n.font_size));
            cmp("fontWeight", json!(o.font_weight), json!(n.font_weight));
            cmp("textAlign", field(o, "textAlign"), field(n, "textAlign"));
            continue;
        }
        if matches!(ko, NodeKind::Frame | NodeKind::Rect) {
            cmp("radius", field(o, "radius"), field(n, "radius"));
        }
        cmp("border", border(o), border(n));
        cmp("shadows", field(o, "shadows"), field(n, "shadows"));
        cmp("gradient", field(o, "gradient"), field(n, "gradient"));
        cmp("image", field(o, "image"), field(n, "image"));
        if ko == NodeKind::Frame {
            cmp("layout", field(o, "layout"), field(n, "layout"));
            cmp("clip", json!(o.clip), json!(n.clip));
            if o.layout != LayoutMode::None {
                cmp("gap", json!(o.gap), json!(n.gap));
                cmp(
                    "padding",
                    json!(format!("{:?}", o.pad())),
                    json!(format!("{:?}", n.pad())),
                );
                cmp("align", field(o, "align"), field(n, "align"));
                cmp("justify", field(o, "justify"), field(n, "justify"));
            }
        }
    }
    out
}
