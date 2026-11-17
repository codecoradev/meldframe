//! Sizes: fixed from the node, fill from the parent, text from the measurer, hug frames from
//! their children.

use super::Layout;
use super::fill::fill_share;
use super::tree::is_layout_frame;
use meldframe_protocol::{LayoutMode, NodeKind, SizeMode};

impl Layout<'_> {
    /// The size of node `i`. `aw` and `ah` are the sizes its parent gives a child that fills that
    /// axis; without one, a fill axis keeps the node's own `w` or `h`. Hug height is computed from
    /// the final width, so text wraps at the width it will really have. Memoised: a hug parent asks
    /// for the same child more than once.
    pub(super) fn size_of(&mut self, i: usize, aw: Option<f64>, ah: Option<f64>) -> (f64, f64) {
        let key = (i, aw.map(bits), ah.map(bits));
        if let Some(v) = self.memo.get(&key) {
            return *v;
        }
        let nodes = self.nodes;
        let n = &nodes[i];
        let mut w = match (n.sw, aw) {
            (SizeMode::Fill, Some(a)) => a,
            _ => n.w,
        };
        let mut h = match (n.sh, ah) {
            (SizeMode::Fill, Some(a)) => a,
            _ => n.h,
        };
        if n.kind == NodeKind::Text {
            if n.sw == SizeMode::Hug {
                w = self.measurer.measure(n, None).0;
            }
            if n.sh == SizeMode::Hug {
                let wrap = if n.sw == SizeMode::Hug { None } else { Some(w) };
                h = self.measurer.measure(n, wrap).1;
            }
        } else if is_layout_frame(n) {
            if n.sw == SizeMode::Hug {
                w = self.hug_width(i);
            }
            if n.sh == SizeMode::Hug {
                h = self.hug_height(i, w);
            }
        }
        self.memo.insert(key, (w, h));
        (w, h)
    }

    /// Width of a hug frame: children summed on a row, the widest on a column, plus padding.
    fn hug_width(&mut self, i: usize) -> f64 {
        let n = &self.nodes[i];
        let row = n.layout == LayoutMode::Row;
        let mut main = 0.0;
        let mut cross: f64 = 0.0;
        for (k, c) in self.flow_children(&n.id).into_iter().enumerate() {
            let s = self.size_of(c, None, None);
            let m = self.nodes[c].margin();
            let (along, across) = if row {
                (s.0 + m.horizontal(), s.1 + m.vertical())
            } else {
                (s.1 + m.vertical(), s.0 + m.horizontal())
            };
            main += along + if k > 0 { n.gap } else { 0.0 };
            cross = cross.max(across);
        }
        (if row { main } else { cross }) + n.pad().horizontal()
    }

    /// Height of a hug frame whose width is `w`. A fill-width child is sized at the width it will
    /// get inside the frame, because that decides how far its text wraps.
    fn hug_height(&mut self, i: usize, w: f64) -> f64 {
        let nodes = self.nodes;
        let n = &nodes[i];
        let pad = n.pad().vertical();
        let inner_w = w - n.pad().horizontal();
        let kids = self.flow_children(&n.id);
        if kids.is_empty() {
            return pad;
        }
        let gaps = n.gap * (kids.len() - 1) as f64;
        if n.layout == LayoutMode::Column {
            let mut total = 0.0;
            for &c in &kids {
                let m = nodes[c].margin();
                let aw = (nodes[c].sw == SizeMode::Fill).then_some(inner_w - m.horizontal());
                total += self.size_of(c, aw, None).1 + m.vertical();
            }
            return total + gaps + pad;
        }
        let mut fixed = 0.0;
        let mut fills = 0usize;
        for &c in &kids {
            fixed += nodes[c].margin().horizontal();
            if nodes[c].sw == SizeMode::Fill {
                fills += 1;
            } else {
                fixed += self.size_of(c, None, None).0;
            }
        }
        let share = fill_share(inner_w, fixed, gaps, fills);
        let mut tallest: f64 = 0.0;
        for &c in &kids {
            let aw = (nodes[c].sw == SizeMode::Fill).then_some(share);
            tallest = tallest.max(self.size_of(c, aw, None).1 + nodes[c].margin().vertical());
        }
        tallest + pad
    }
}

/// The bit pattern of `v` with negative zero folded into zero, so equal sizes share a memo entry.
pub(super) fn bits(v: f64) -> u64 {
    (v + 0.0).to_bits()
}
