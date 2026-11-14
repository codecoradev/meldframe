//! Placement: turn sizes into boxes, parents before children.

use super::tree::is_layout_frame;
use super::{Layout, Rect};
use meldframe_protocol::{LayoutMode, NodeKind};

impl Layout<'_> {
    /// Record node `i` at `r`, then place its children inside it.
    fn place(&mut self, i: usize, r: Rect) {
        let nodes = self.nodes;
        let n = &nodes[i];
        self.out.push((i, r));
        if n.kind != NodeKind::Frame {
            return;
        }
        let kids = self.children(&n.id);
        if !is_layout_frame(n) {
            for c in kids {
                let s = self.size_of(c);
                let cn = &nodes[c];
                self.place(
                    c,
                    Rect {
                        x: r.x + cn.x,
                        y: r.y + cn.y,
                        w: s.0,
                        h: s.1,
                    },
                );
            }
            return;
        }
        let row = n.layout == LayoutMode::Row;
        let mut pos = if row { r.x } else { r.y } + n.padding;
        let cross_pos = if row { r.y } else { r.x } + n.padding;
        for c in kids {
            let (cw, ch) = self.size_of(c);
            let rect = if row {
                Rect {
                    x: pos,
                    y: cross_pos,
                    w: cw,
                    h: ch,
                }
            } else {
                Rect {
                    x: cross_pos,
                    y: pos,
                    w: cw,
                    h: ch,
                }
            };
            self.place(c, rect);
            pos += if row { cw } else { ch } + n.gap;
        }
    }

    /// Place every root at its own position; returns the boxes in drawing order.
    pub(super) fn run(mut self) -> Vec<(usize, Rect)> {
        for i in self.roots() {
            let (w, h) = self.size_of(i);
            let n = &self.nodes[i];
            self.place(
                i,
                Rect {
                    x: n.x,
                    y: n.y,
                    w,
                    h,
                },
            );
        }
        self.out
    }
}
