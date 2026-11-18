//! Layout findings: the validator rules of the fidelity contract (G1 to G3). They report, they never change the
//! layout: the core lays a node out the same way with or without a finding on it.
//!
//! - `fill_under_hug` (G1): `fill` on an axis where the parent hugs, or where the parent has no layout. The export
//!   treats it as fixed. The canvas keeps the node's own size in a frame without layout, and sizes it from what a hug
//!   layout frame hugs (an equal share on the main axis, the whole on the cross axis); the finding says which.
//! - `overflow` (G3): the flow children of a fixed-size layout frame, with their margins and the gaps, are larger
//!   than its content box.
//! - `zero_fill` (G3): a `fill` that resolves to less than 1 px.
//! - `smaller_than_padding` (G3): a layout frame smaller than its own padding.
//!
//! A frame whose children include hug text, directly or through hugging layout frames, is left out of the size rules:
//! the text measurer of the caller may not be the browser's, so the fit of such a frame cannot be told here.

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

/// The findings for a file, in the order of the nodes. Frames with hug text among their children, directly or through
/// hugging layout frames, are skipped by the size rules.
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
        let row = parent.layout == LayoutMode::Row;
        for (axis, mode, parent_mode, main) in [
            ("width", n.sw, parent.sw, row),
            ("height", n.sh, parent.sh, !row),
        ] {
            if mode == SizeMode::Fill && (!layout || parent_mode == SizeMode::Hug) {
                // In a hug layout frame the canvas still sizes the child from what the parent hugs: an equal share
                // of it on the main axis, the whole of it on the cross axis (as CSS `align-self: stretch` does). In a
                // frame without layout the child keeps its own size. The export treats both as fixed (rule G1).
                let canvas = if layout {
                    let how = if main {
                        format!(
                            "a share of the {axis} the parent hugs (its own {axis} when it is the only fill)"
                        )
                    } else {
                        format!("the whole of the {axis} the parent hugs")
                    };
                    format!("the canvas gives it {how}, while the export keeps its own {axis}")
                } else {
                    format!("it keeps its own {axis}")
                };
                out.push(finding(n, "fill_under_hug", format!("{} is fill on the {axis}, but its parent does not give it a size of its own there (the parent hugs, or has no layout): {canvas}", n.name)));
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
        if has_hug_text(&layout, nodes, &kids) {
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

/// Whether the size of any of `kids` depends on a text the caller's measurer sizes: a hug text among them, or a
/// hugging frame whose own flow children reach one.
fn has_hug_text(layout: &Layout, nodes: &[Node], kids: &[usize]) -> bool {
    kids.iter().any(|&c| {
        let k = &nodes[c];
        let hugs = k.sw == SizeMode::Hug || k.sh == SizeMode::Hug;
        match k.kind {
            NodeKind::Text => hugs,
            // A hug frame without layout keeps its own size (the core treats its hug as fixed).
            NodeKind::Frame => {
                hugs && is_layout_frame(k)
                    && has_hug_text(layout, nodes, &layout.flow_children(&k.id))
            }
            _ => false,
        }
    })
}
