//! Sizes: fixed from the node, text from the measurer, hug frames from their children.

use super::Layout;
use super::tree::is_layout_frame;
use meldframe_protocol::{LayoutMode, NodeKind, SizeMode};

impl Layout<'_> {
    /// The size of node `i`. Memoised: a hug parent asks for the same child more than once.
    pub(super) fn size_of(&mut self, i: usize) -> (f64, f64) {
        if let Some(v) = self.memo.get(&i) {
            return *v;
        }
        let nodes = self.nodes;
        let n = &nodes[i];
        let mut w = n.w;
        let mut h = n.h;
        if n.kind == NodeKind::Text {
            if n.sw == SizeMode::Hug {
                w = self.measurer.measure(n, None).0;
            }
            if n.sh == SizeMode::Hug {
                let wrap = if n.sw == SizeMode::Hug { None } else { Some(w) };
                h = self.measurer.measure(n, wrap).1;
            }
        } else if is_layout_frame(n) && (n.sw == SizeMode::Hug || n.sh == SizeMode::Hug) {
            let row = n.layout == LayoutMode::Row;
            let pad = n.padding * 2.0;
            let mut main = 0.0;
            let mut cross: f64 = 0.0;
            for (k, c) in self.children(&n.id).into_iter().enumerate() {
                let s = self.size_of(c);
                main += if row { s.0 } else { s.1 } + if k > 0 { n.gap } else { 0.0 };
                cross = cross.max(if row { s.1 } else { s.0 });
            }
            let (content_w, content_h) = if row { (main, cross) } else { (cross, main) };
            if n.sw == SizeMode::Hug {
                w = content_w + pad;
            }
            if n.sh == SizeMode::Hug {
                h = content_h + pad;
            }
        }
        self.memo.insert(i, (w, h));
        (w, h)
    }
}
