//! The walk over the frame: the render step, the styles and the document.

use crate::decl::{Style, attrs, style, tag};
use crate::names::escape;
use crate::report::Report;
use crate::{Export, ExportError, ExportOptions};
use meldframe_protocol::{Node, NodeKind, Variables};
use meldframe_render::{PaintedBox, resolve_with};
use std::collections::HashMap;

/// The elements and rules of the frame, in drawing order, built by one walk.
struct Walk<'a> {
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
    let mut w = Walk {
        nodes: by_id,
        kids,
        html: String::new(),
        css: String::new(),
        report: Report::default(),
    };
    // A top-level frame is where it is; a frame inside another is the origin of its own page.
    let top = root
        .parent_id
        .as_deref()
        .is_none_or(|p| !w.nodes.contains_key(p));
    let origin = if top { (root.x, root.y) } else { (0.0, 0.0) };
    w.element(root_box, Some(origin), 2);

    let title = if root.name.is_empty() {
        &root.id
    } else {
        &root.name
    };
    let head_css = base_css(options);
    let style_tag = format!("<style>\n{head_css}{}</style>", w.css);
    let css = format!("{head_css}{}", w.css);
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

/// The font, the box model and the margins every export needs.
fn base_css(options: &ExportOptions<'_>) -> String {
    let font = format!(
        "@font-face{{font-family:'Inter Variable';src:url('{}') format('woff2');font-weight:100 900;font-display:block}}\n",
        options.font_href
    );
    let reset = "*,::before,::after{box-sizing:border-box}body{margin:0;font-family:'Inter Variable'}p{margin:0}";
    format!("{font}{reset}\n")
}

impl<'a> Walk<'a> {
    fn element(&mut self, b: &'a PaintedBox, root: Option<(f64, f64)>, depth: usize) {
        let n = self.nodes[b.id.as_str()];
        let kids: Vec<&PaintedBox> = self.kids.get(b.id.as_str()).cloned().unwrap_or_default();
        let st = style(n, root);
        let t = tag(b);
        let pad = "  ".repeat(depth);
        let open = format!("{pad}<{t} {}", attrs(n));
        self.rules(&n.id, &st);
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
            self.element(k, None, depth + 1);
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
