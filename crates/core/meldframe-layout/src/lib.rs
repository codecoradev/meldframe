//! Layout: the box of every node, from hug, fill and auto layout rules.
//!
//! The layout result is the source of truth for geometry (ADR-0003); export translates it to CSS.
//! Text sizes come through the [`TextMeasurer`] seam so the crate stays free of I/O.

mod contract;
mod fill;
mod findings;
mod measure;
mod place;
mod rect;
mod size;
mod tree;

pub use contract::text_outside_contract;
pub use findings::{LayoutFinding, layout_findings};
pub use measure::{StubMeasurer, TextMeasurer};
pub use rect::Rect;
pub use tree::is_layout_frame;

use meldframe_protocol::Node;
use std::collections::HashMap;

/// A node index with the fill sizes its parent gave it (as bit patterns).
type MemoKey = (usize, Option<u64>, Option<u64>);

/// One layout pass over a flat node list.
struct Layout<'a> {
    nodes: &'a [Node],
    measurer: &'a dyn TextMeasurer,
    kids: HashMap<Option<&'a str>, Vec<usize>>,
    memo: HashMap<MemoKey, (f64, f64)>,
    out: Vec<(usize, Rect)>,
}

impl<'a> Layout<'a> {
    fn new(nodes: &'a [Node], measurer: &'a dyn TextMeasurer) -> Self {
        Layout {
            nodes,
            measurer,
            kids: tree::children_by_parent(nodes),
            memo: HashMap::new(),
            out: Vec::with_capacity(nodes.len()),
        }
    }

    fn children(&self, id: &str) -> Vec<usize> {
        self.kids.get(&Some(id)).cloned().unwrap_or_default()
    }

    /// The children that take part in the flow of an auto layout frame: not the absolute ones.
    fn flow_children(&self, id: &str) -> Vec<usize> {
        let mut kids = self.children(id);
        kids.retain(|&c| !self.nodes[c].absolute);
        kids
    }

    fn roots(&self) -> Vec<usize> {
        self.kids.get(&None).cloned().unwrap_or_default()
    }
}

/// The box of every node reachable from a root, as indexes into `nodes`, in drawing order
/// (parent before children, siblings by position then id). Copies no strings, so it suits the hot
/// path in WASM.
pub fn compute_rects_indexed(nodes: &[Node], measurer: &dyn TextMeasurer) -> Vec<(usize, Rect)> {
    Layout::new(nodes, measurer).run()
}

/// Like [`compute_rects_indexed`], with node ids.
pub fn compute_rects(nodes: &[Node], measurer: &dyn TextMeasurer) -> Vec<(String, Rect)> {
    compute_rects_indexed(nodes, measurer)
        .into_iter()
        .map(|(i, r)| (nodes[i].id.clone(), r))
        .collect()
}
