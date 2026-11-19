//! The walk over the frame: the render step, the styles, the variables, and the document.

use crate::decl::{Cx, Place, Style, attrs, style, tag};
use crate::names::escape;
use crate::report::Report;
use crate::{Export, ExportError, ExportOptions, Format, tw};
use meldframe_layout::{Rect, compute_rects, layout_findings};
use meldframe_protocol::{LayoutMode, Node, NodeKind, SizeMode, Variables};
use meldframe_render::{PaintedBox, resolve_with};
use std::collections::{HashMap, HashSet};

/// The elements and rules of the frame, in drawing order, built by one walk.
struct Walk<'a> {
    cx: &'a Cx<'a>,
    options: &'a ExportOptions<'a>,
    nodes: HashMap<&'a str, &'a Node>,
    kids: HashMap<&'a str, Vec<&'a PaintedBox>>,
    html: String,
    css: String,
    report: Report,
}

pub(crate) fn export(
    nodes: &[Node],
    vars_in: &Variables,
    frame: &str,
    options: &ExportOptions<'_>,
) -> Result<Export, ExportError> {
    let root = nodes
        .iter()
        .find(|n| n.id == frame)
        .ok_or(ExportError::NotFound)?;
    if root.kind != NodeKind::Frame {
        return Err(ExportError::NotAFrame);
    }
    // The one step the canvas reads too: nodes, variables and the modes picked to painted boxes.
    let boxes = resolve_with(nodes, vars_in, &options.modes);
    let mut kids: HashMap<&str, Vec<&PaintedBox>> = HashMap::new();
    for b in &boxes {
        if let Some(p) = &b.parent {
            kids.entry(p.as_str()).or_default().push(b);
        }
    }
    let by_id: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let root_box = boxes
        .iter()
        .find(|b| b.id == frame)
        .ok_or(ExportError::NotFound)?;
    let mut order = Vec::new();
    collect(root_box, &kids, &mut HashSet::new(), &mut order);

    let mut report = Report::default();
    let in_frame: HashSet<&str> = order.iter().map(|b| b.id.as_str()).collect();
    let rects: HashMap<String, Rect> = compute_rects(nodes, options.measurer).into_iter().collect();
    for f in layout_findings(nodes, options.measurer) {
        if in_frame.contains(f.id.as_str()) {
            report.warn(f.code, Some(&f.id), f.message);
        }
    }
    let cx = Cx {
        rects: &rects,
        measurer: options.measurer,
        asset_base: &options.asset_base,
    };
    let mut w = Walk {
        cx: &cx,
        options,
        nodes: by_id,
        kids,
        html: String::new(),
        css: String::new(),
        report,
    };
    // A top-level frame is where it is; a frame inside another is the origin of its own page.
    let top = root
        .parent_id
        .as_deref()
        .is_none_or(|p| !w.nodes.contains_key(p));
    let origin = if top { (root.x, root.y) } else { (0.0, 0.0) };
    w.element(root_box, None, Some(origin), 2);

    let title = if root.name.is_empty() {
        &root.id
    } else {
        &root.name
    };
    let base = base_css(options, options.format == Format::Tailwind);
    let head_css = base;
    let (style_tag, css) = match options.format {
        Format::Css => (
            format!("<style>\n{head_css}{}</style>", w.css),
            format!("{head_css}{}", w.css),
        ),
        Format::Tailwind => (
            format!(
                "<link rel=\"stylesheet\" href=\"{}\">",
                escape(&options.css_href)
            ),
            format!("{TAILWIND_HEAD}{head_css}"),
        ),
    };
    let html = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n{style_tag}\n</head>\n<body>\n{}</body>\n</html>\n",
        escape(title),
        w.html
    );
    let bytes = html.len() + css.len();
    if bytes > options.max_bytes {
        return Err(ExportError::TooLarge {
            bytes,
            max: options.max_bytes,
        });
    }
    Ok(Export {
        html,
        css,
        report: w.report,
    })
}

/// The frame and its descendants, parent before children, siblings as the render step ordered them.
fn collect<'a>(
    b: &'a PaintedBox,
    kids: &HashMap<&str, Vec<&'a PaintedBox>>,
    seen: &mut HashSet<&'a str>,
    out: &mut Vec<&'a PaintedBox>,
) {
    if seen.insert(&b.id) {
        out.push(b);
        for k in kids.get(b.id.as_str()).into_iter().flatten() {
            collect(k, kids, seen, out);
        }
    }
}

const TAILWIND_HEAD: &str = "@layer theme, base, utilities;\n@import \"tailwindcss/theme.css\" layer(theme);\n@import \"tailwindcss/utilities.css\" layer(utilities);\n";

/// The font, the box model and the margins every export needs. In the Tailwind form the reset is in the `base` layer,
/// so that a utility on a paragraph still wins over `p{margin:0}`.
fn base_css(options: &ExportOptions<'_>, layered: bool) -> String {
    let font = format!(
        "@font-face{{font-family:'Inter Variable';src:url('{}') format('woff2');font-weight:100 900;font-display:block}}\n",
        options.font_href
    );
    let reset = "*,::before,::after{box-sizing:border-box}body{margin:0;font-family:'Inter Variable'}p{margin:0}";
    if layered {
        format!("{font}@layer base{{{reset}}}\n")
    } else {
        format!("{font}{reset}\n")
    }
}

impl<'a> Walk<'a> {
    fn element(
        &mut self,
        b: &'a PaintedBox,
        parent: Option<&'a Node>,
        root: Option<(f64, f64)>,
        depth: usize,
    ) {
        let n = self.nodes[b.id.as_str()];
        let kids: Vec<&PaintedBox> = self.kids.get(b.id.as_str()).cloned().unwrap_or_default();
        let holds_absolute = n.kind == NodeKind::Frame
            && kids.iter().any(|k| {
                let kn = self.nodes[k.id.as_str()];
                n.layout == LayoutMode::None || kn.absolute
            });
        let basis = parent.and_then(|p| fill_basis(p, n, &self.kids, &self.nodes, self.cx.rects));
        let place = Place {
            parent,
            root,
            holds_absolute,
            basis,
        };
        let st = style(self.cx, n, b, &place, &mut self.report);
        let t = tag(b);
        let pad = "  ".repeat(depth);
        let mut open = format!("{pad}<{t} {}", attrs(n));
        match self.options.format {
            Format::Css => self.rules(&n.id, &st),
            Format::Tailwind => {
                let classes: Vec<String> = st
                    .main
                    .iter()
                    .map(|d| tw::class(d, false))
                    .chain(st.after.iter().map(|d| tw::class(d, true)))
                    .collect();
                open.push_str(&format!(" class=\"{}\"", escape(&classes.join(" "))));
            }
        }
        if b.paint == meldframe_render::Paint::Text {
            self.html
                .push_str(&format!("{open}>{}</{t}>\n", escape(&n.text)));
            return;
        }
        if kids.is_empty() {
            self.html.push_str(&format!("{open}></{t}>\n"));
            return;
        }
        self.html.push_str(&format!("{open}>\n"));
        for k in kids {
            self.element(k, Some(n), None, depth + 1);
        }
        self.html.push_str(&format!("{pad}</{t}>\n"));
    }

    fn rules(&mut self, id: &str, st: &Style) {
        for (suffix, decls) in [("", &st.main), ("::after", &st.after)] {
            if !decls.is_empty() {
                let body: Vec<String> = decls
                    .iter()
                    .map(|d| format!("{}:{}", d.prop, d.value))
                    .collect();
                self.css.push_str(&format!(
                    "[data-node=\"{id}\"]{suffix}{{{}}}\n",
                    body.join(";")
                ));
            }
        }
    }
}

/// The size a fill takes on the main axis of `p` when the fills of that axis have unequal padding (the contract's
/// `flex:1 1 <core px>`): the core's size of the node, else `None`, which is `0%`.
fn fill_basis(
    p: &Node,
    n: &Node,
    kids: &HashMap<&str, Vec<&PaintedBox>>,
    nodes: &HashMap<&str, &Node>,
    rects: &HashMap<String, Rect>,
) -> Option<f64> {
    let row = p.layout == LayoutMode::Row;
    let mode = |c: &Node| if row { c.sw } else { c.sh };
    let parent_mode = if row { p.sw } else { p.sh };
    if p.kind != NodeKind::Frame
        || p.layout == LayoutMode::None
        || n.absolute
        || mode(n) != SizeMode::Fill
        || parent_mode == SizeMode::Hug
    {
        return None;
    }
    let pad = |c: &Node| {
        let q = c.pad();
        if c.kind == NodeKind::Frame && c.layout != LayoutMode::None {
            if row { q.horizontal() } else { q.vertical() }
        } else {
            0.0
        }
    };
    let fills: Vec<&Node> = kids
        .get(p.id.as_str())?
        .iter()
        .filter_map(|b| nodes.get(b.id.as_str()).copied())
        .filter(|c| !c.absolute && mode(c) == SizeMode::Fill)
        .collect();
    let equal = fills.iter().all(|c| pad(c) == pad(n));
    let r = rects.get(n.id.as_str())?;
    (!equal).then_some(if row { r.w } else { r.h })
}
