//! Layout findings: the validator rules of the fidelity contract (G1 to G3). They report, they never change the
//! layout: the core lays a node out the same way with or without a finding on it.
//!
//! - `fill_under_hug` (G1): `fill` on an axis where the parent hugs, or where the parent has no layout. The core
//!   uses the node's own size there, so the fill does nothing; the export treats it as fixed.
//! - `overflow` (G3): the flow children of a fixed-size layout frame, with their margins and the gaps, are larger
//!   than its content box.
//! - `zero_fill` (G3): a `fill` that resolves to less than 1 px.
//! - `smaller_than_padding` (G3): a layout frame smaller than its own padding.
//!
//! A frame whose children include hug text is left out of the size rules: the text measurer of the caller may not be
//! the browser's, so the fit of such a frame cannot be told here.

use super::tree::is_layout_frame;
use super::{Layout, Rect, TextMeasurer};
use meldframe_protocol::{LayoutMode, Node, NodeKind, SizeMode};
use std::collections::HashMap;

/// A problem a person or an agent should look at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutFinding {
    pub id: String,
    /// `fill_under_hug`, `overflow`, `zero_fill` or `smaller_than_padding`.
    pub code: &'static str,
    pub message: String,
}

fn finding(n: &Node, code: &'static str, message: String) -> LayoutFinding {
    LayoutFinding {
        id: n.id.clone(),
        code,
        message,
    }
}

/// The findings for a file, in the order of the nodes. Frames with hug text children are skipped by the size rules.
pub fn layout_findings(nodes: &[Node], measurer: &dyn TextMeasurer) -> Vec<LayoutFinding> {
    let rects: HashMap<usize, Rect> = Layout::new(nodes, measurer).run().into_iter().collect();
    let by_id: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let mut out = Vec::new();
    for n in nodes {
        let Some(&pi) = n.parent_id.as_deref().and_then(|p| by_id.get(p)) else {
            continue;
        };
        let parent = &nodes[pi];
        if parent.kind != NodeKind::Frame || n.absolute {
            continue;
        }
        let layout = is_layout_frame(parent);
        for (axis, mode, parent_mode) in [("width", n.sw, parent.sw), ("height", n.sh, parent.sh)] {
            if mode == SizeMode::Fill && (!layout || parent_mode == SizeMode::Hug) {
                out.push(finding(n, "fill_under_hug", format!("{} is fill on the {axis}, but its parent does not give it a size there (the parent hugs, or has no layout): it keeps its own {axis}", n.name)));
            }
        }
    }
    let layout = Layout::new(nodes, measurer);
    for (i, n) in nodes.iter().enumerate() {
        if !is_layout_frame(n) {
            continue;
        }
        let Some(r) = rects.get(&i) else { continue };
        let kids = layout.flow_children(&n.id);
        if kids.iter().any(|&c| {
            nodes[c].kind == NodeKind::Text
                && (nodes[c].sw == SizeMode::Hug || nodes[c].sh == SizeMode::Hug)
        }) {
            continue;
        }
        let pad = n.pad();
        if r.w < pad.horizontal() || r.h < pad.vertical() {
            out.push(finding(
                n,
                "smaller_than_padding",
                format!(
                    "{} is smaller than its own padding ({} x {} for a padding of {} x {})",
                    n.name,
                    r.w,
                    r.h,
                    pad.horizontal(),
                    pad.vertical()
                ),
            ));
        }
        let row = n.layout == LayoutMode::Row;
        let hug_main = if row { n.sw } else { n.sh } == SizeMode::Hug;
        if !hug_main && !kids.is_empty() {
            let inner = if row {
                r.w - pad.horizontal()
            } else {
                r.h - pad.vertical()
            };
            let mut used = n.gap * (kids.len() - 1) as f64;
            for &c in &kids {
                let m = nodes[c].margin();
                if let Some(cr) = rects.get(&c) {
                    used += if row {
                        cr.w + m.horizontal()
                    } else {
                        cr.h + m.vertical()
                    };
                }
            }
            if used > inner + 1e-6 {
                out.push(finding(n, "overflow", format!("The children of {} need {used} but its content box is {inner} along the {} axis", n.name, if row { "horizontal" } else { "vertical" })));
            }
        }
        for &c in &kids {
            let (fw, fh) = (nodes[c].sw == SizeMode::Fill, nodes[c].sh == SizeMode::Fill);
            if let Some(cr) = rects.get(&c) {
                let main_fill = if row { fw } else { fh };
                let cross_fill = if row { fh } else { fw };
                let (main, cross) = if row { (cr.w, cr.h) } else { (cr.h, cr.w) };
                if (main_fill && main < 1.0) || (cross_fill && cross < 1.0) {
                    out.push(finding(
                        &nodes[c],
                        "zero_fill",
                        format!("{} is fill but resolves to less than 1 px", nodes[c].name),
                    ));
                }
            }
        }
    }
    out
}
