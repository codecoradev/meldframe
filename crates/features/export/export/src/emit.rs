//! The walk over the frame: the render step, the styles, the variables, and the document.

use crate::decl::{Cx, Place, Style, attrs, style, tag};
use crate::names::{css_escape, escape};
use crate::report::Report;
use crate::{Export, ExportError, ExportOptions, Format, tw, vars};
use meldframe_layout::{Rect, compute_rects, layout_findings};
use meldframe_protocol::{LayoutMode, Node, NodeKind, SizeMode, Variables};
use meldframe_render::{Fields, ModeChoice, PaintedBox, resolve_with};
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
    /// The texts whose size the measurer decides (only when the options say it is an estimate).
    measured: Vec<&'a str>,
}

/// The most ids the `text_size_estimated` message lists.
const ESTIMATED_LISTED: usize = 8;

/// One warning for the texts whose box the measurer decides, in the order of the walk: [`Style::measured`] is set by
/// the one place that pins a text from the measurer, so this list cannot drift from the declarations. A text of fixed
/// size is not listed.
fn estimated_text(ids: &[&str], report: &mut Report) {
    if ids.is_empty() {
        return;
    }
    let more = ids.len().saturating_sub(ESTIMATED_LISTED);
    let tail = if more > 0 {
        format!(" and {more} more")
    } else {
        String::new()
    };
    let listed = ids[..ids.len() - more].join(", ");
    let what = match ids.len() {
        1 => "1 text box gets its size".to_owned(),
        n => format!("{n} text boxes get their size"),
    };
    report.warn(
        "text_size_estimated",
        None,
        format!(
            "{what} from an estimate of the text, not from the font the browser uses, so the size may differ from the canvas and the Export dialog: {listed}{tail}"
        ),
    );
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
    // Everything below works on the frame and what is inside it, never on the rest of the file.
    let mut report = Report::default();
    let sub = subtree(nodes, root, options.max_bytes, &mut report)?;
    check_modes(vars_in, &options.modes, &mut report);
    // The one step the canvas reads too: nodes, variables and the modes picked to painted boxes.
    let boxes = resolve_with(&sub, vars_in, &options.modes);
    let mut kids: HashMap<&str, Vec<&PaintedBox>> = HashMap::new();
    for b in &boxes {
        if let Some(p) = &b.parent {
            kids.entry(p.as_str()).or_default().push(b);
        }
    }
    let by_id: HashMap<&str, &Node> = sub.iter().map(|n| (n.id.as_str(), n)).collect();
    let root_box = boxes
        .iter()
        .find(|b| b.id == frame)
        .ok_or(ExportError::NotFound)?;
    let mut order = Vec::new();
    collect(root_box, &kids, &mut HashSet::new(), &mut order, 0);

    let fields = Fields::new(vars_in, &options.modes);
    let used: HashSet<String> = order
        .iter()
        .filter_map(|b| by_id.get(b.id.as_str()))
        .flat_map(|n| [n.fill.var_id(), n.radius.var_id(), n.opacity.var_id()])
        .flatten()
        .map(str::to_owned)
        .collect();
    let var_css = vars::build(vars_in, &used, &options.modes, &fields, &mut report);
    let rects: HashMap<String, Rect> = compute_rects(&sub, options.measurer).into_iter().collect();
    for f in layout_findings(&sub, options.measurer) {
        report.warn(f.code, Some(&f.id), f.message);
    }
    let cx = Cx {
        names: &var_css.names,
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
        measured: Vec::new(),
    };
    // A top-level frame is where it is; a frame inside another is the origin of its own page.
    let top = root
        .parent_id
        .as_deref()
        .is_none_or(|p| !nodes.iter().any(|n| n.id == p));
    let origin = if top { (root.x, root.y) } else { (0.0, 0.0) };
    w.element(root_box, None, Some(origin), 2, false)?;
    estimated_text(&w.measured, &mut w.report);

    let title = if root.name.is_empty() {
        &root.id
    } else {
        &root.name
    };
    let attr_text: String = var_css
        .attrs
        .iter()
        .map(|(k, v)| format!(" {k}=\"{v}\""))
        .collect();
    let base = base_css(options, options.format == Format::Tailwind);
    let head_css = format!("{base}{}", var_css.rules);
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
        "<!doctype html>\n<html lang=\"en\"{attr_text}>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n{style_tag}\n</head>\n<body>\n{}</body>\n</html>\n",
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
    depth: usize,
) {
    if depth <= MAX_DEPTH && seen.insert(&b.id) {
        out.push(b);
        for k in kids.get(b.id.as_str()).into_iter().flatten() {
            collect(k, kids, seen, out, depth + 1);
        }
    }
}

/// The deepest nesting an export follows below the frame, whatever the file says: the writes of the server stop at 32,
/// a file that came from elsewhere may nest deeper, and the walks below recurse.
pub(crate) const MAX_DEPTH: usize = 64;

/// What an element costs at the least, in bytes of HTML and stylesheet: `<div data-node="" class=""></div>`.
const MIN_ELEMENT_BYTES: usize = 24;

/// The frame and what is inside it, in the order of the file, with the frame as a root. The walk is iterative and
/// stops at [`MAX_DEPTH`] (the cut is a `too_deep` warning). It also counts what the output must hold at the least
/// (the text, the names, a floor per element) and refuses as soon as that is over the limit, so that a frame that
/// cannot fit is never cloned, laid out or painted.
fn subtree(
    nodes: &[Node],
    root: &Node,
    max_bytes: usize,
    report: &mut Report,
) -> Result<Vec<Node>, ExportError> {
    let mut kids: HashMap<&str, Vec<&Node>> = HashMap::new();
    for n in nodes {
        if let Some(p) = n.parent_id.as_deref() {
            kids.entry(p).or_default().push(n);
        }
    }
    let mut keep: HashSet<&str> = HashSet::new();
    let mut floor = 0usize;
    let mut cut = false;
    let mut todo: Vec<(&Node, usize)> = vec![(root, 0)];
    while let Some((n, depth)) = todo.pop() {
        if depth > MAX_DEPTH {
            if !cut {
                report.warn(
                    "too_deep",
                    Some(&n.id),
                    format!("the frame nests deeper than {MAX_DEPTH} levels; everything from here down was left out"),
                );
            }
            cut = true;
            continue;
        }
        if !keep.insert(n.id.as_str()) {
            continue;
        }
        floor += MIN_ELEMENT_BYTES + n.id.len() + n.name.len();
        if n.kind == NodeKind::Text {
            floor += n.text.len();
        }
        if floor > max_bytes {
            return Err(ExportError::TooLarge {
                bytes: floor,
                max: max_bytes,
            });
        }
        for k in kids.get(n.id.as_str()).into_iter().flatten() {
            todo.push((k, depth + 1));
        }
    }
    Ok(nodes
        .iter()
        .filter(|n| keep.contains(n.id.as_str()))
        .map(|n| {
            let mut n = n.clone();
            if n.id == root.id {
                n.parent_id = None;
            }
            n
        })
        .collect())
}

/// A collection or a mode of the options that is not in the file says so; the export shows the default mode there.
fn check_modes(vars: &Variables, choice: &ModeChoice, report: &mut Report) {
    for (collection, mode) in choice.iter() {
        let known = vars
            .collection(collection)
            .is_some_and(|c| c.modes.iter().any(|m| m.id == mode));
        if !known {
            report.warn(
                "unknown_mode",
                None,
                "a collection or mode in `modes` is not in the file; its default mode is exported"
                    .into(),
            );
        }
    }
}

const TAILWIND_HEAD: &str = "@layer theme, base, utilities;\n@import \"tailwindcss/theme.css\" layer(theme);\n@import \"tailwindcss/utilities.css\" layer(utilities);\n";

/// The font, the box model and the margins every export needs. In the Tailwind form the reset is in the `base` layer,
/// so that a utility on a paragraph still wins over `p{margin:0}`.
fn base_css(options: &ExportOptions<'_>, layered: bool) -> String {
    let font = format!(
        "@font-face{{font-family:'Inter Variable';src:url('{}') format('woff2');font-weight:100 900;font-display:block}}\n",
        css_escape(&options.font_href, "-_./:")
    );
    let reset = "*,::before,::after{box-sizing:border-box}body{margin:0;font-family:'Inter Variable'}p{margin:0}";
    if layered {
        format!("{font}@layer base{{{reset}}}\n")
    } else {
        format!("{font}{reset}\n")
    }
}

impl<'a> Walk<'a> {
    /// The bytes the answer will carry so far: the plain CSS form has the stylesheet in the document and in `css`.
    fn size(&self) -> usize {
        let sheets = if self.options.format == Format::Css {
            2
        } else {
            0
        };
        self.html.len() + sheets * self.css.len()
    }

    fn element(
        &mut self,
        b: &'a PaintedBox,
        parent: Option<&'a Node>,
        root: Option<(f64, f64)>,
        depth: usize,
        unequal: bool,
    ) -> Result<(), ExportError> {
        if depth > MAX_DEPTH + 2 {
            return Ok(());
        }
        if self.size() > self.options.max_bytes {
            return Err(ExportError::TooLarge {
                bytes: self.size(),
                max: self.options.max_bytes,
            });
        }
        let n = self.nodes[b.id.as_str()];
        let kids: Vec<&PaintedBox> = self.kids.get(b.id.as_str()).cloned().unwrap_or_default();
        let holds_absolute = n.kind == NodeKind::Frame
            && kids.iter().any(|k| {
                let kn = self.nodes[k.id.as_str()];
                n.layout == LayoutMode::None || kn.absolute
            });
        let basis = parent.and_then(|p| fill_basis(p, n, unequal, self.cx.rects));
        let place = Place {
            parent,
            root,
            holds_absolute,
            basis,
        };
        let st = style(self.cx, n, b, &place, &mut self.report);
        if st.measured && self.options.estimated_text {
            self.measured.push(n.id.as_str());
        }
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
            return Ok(());
        }
        if kids.is_empty() {
            self.html.push_str(&format!("{open}></{t}>\n"));
            return Ok(());
        }
        self.html.push_str(&format!("{open}>\n"));
        // Whether the fills of this parent have unequal padding is one answer for all of its children.
        let unequal = unequal_fills(n, &kids, &self.nodes);
        for k in kids {
            self.element(k, Some(n), None, depth + 1, unequal)?;
        }
        self.html.push_str(&format!("{pad}</{t}>\n"));
        Ok(())
    }

    fn rules(&mut self, id: &str, st: &Style) {
        let id = css_escape(id, "-_");
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
/// `flex:1 1 <core px>`): the core's size of the node, else `None`, which is `0%`. `unequal` is the answer of
/// [`unequal_fills`] for `p`, worked out once per parent.
fn fill_basis(p: &Node, n: &Node, unequal: bool, rects: &HashMap<String, Rect>) -> Option<f64> {
    let row = p.layout == LayoutMode::Row;
    let mode = |c: &Node| if row { c.sw } else { c.sh };
    if !is_flow_parent(p) || n.absolute || mode(n) != SizeMode::Fill {
        return None;
    }
    let r = rects.get(n.id.as_str())?;
    unequal.then_some(if row { r.w } else { r.h })
}

/// A frame whose fills on the main axis are sized by `flex` (it lays out, and does not hug on that axis).
fn is_flow_parent(p: &Node) -> bool {
    let row = p.layout == LayoutMode::Row;
    let parent_mode = if row { p.sw } else { p.sh };
    p.kind == NodeKind::Frame && p.layout != LayoutMode::None && parent_mode != SizeMode::Hug
}

/// Whether the fills of `p` on its main axis do not all have the same padding on that axis.
fn unequal_fills(p: &Node, kids: &[&PaintedBox], nodes: &HashMap<&str, &Node>) -> bool {
    if !is_flow_parent(p) {
        return false;
    }
    let row = p.layout == LayoutMode::Row;
    let mode = |c: &Node| if row { c.sw } else { c.sh };
    let pad = |c: &Node| {
        let q = c.pad();
        if c.kind == NodeKind::Frame && c.layout != LayoutMode::None {
            if row { q.horizontal() } else { q.vertical() }
        } else {
            0.0
        }
    };
    let mut first = None;
    for c in kids
        .iter()
        .filter_map(|b| nodes.get(b.id.as_str()).copied())
        .filter(|c| !c.absolute && mode(c) == SizeMode::Fill)
    {
        match first {
            None => first = Some(pad(c)),
            Some(f) if f != pad(c) => return true,
            Some(_) => {}
        }
    }
    false
}
