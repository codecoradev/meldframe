//! Placement: turn sizes into boxes, parents before children.

use super::fill::{fill_share, spread};
use super::tree::is_layout_frame;
use super::{Layout, Rect};
use meldframe_protocol::{Align, LayoutMode, NodeKind, SizeMode};

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
                let s = self.size_of(c, None, None);
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
        let inner_w = r.w - n.padding * 2.0;
        let inner_h = r.h - n.padding * 2.0;
        let (inner_main, inner_cross) = if row {
            (inner_w, inner_h)
        } else {
            (inner_h, inner_w)
        };
        let main_mode = |k: usize| if row { nodes[k].sw } else { nodes[k].sh };
        let cross_mode = |k: usize| if row { nodes[k].sh } else { nodes[k].sw };

        // Stage 1: size every child with only the cross axis assigned.
        let mut items: Vec<(usize, (f64, f64))> = Vec::with_capacity(kids.len());
        for &c in &kids {
            let cross = (cross_mode(c) == SizeMode::Fill).then_some(inner_cross);
            let s = if row {
                self.size_of(c, None, cross)
            } else {
                self.size_of(c, cross, None)
            };
            items.push((c, s));
        }
        let mut fixed = 0.0;
        let mut fills = 0usize;
        for &(c, s) in &items {
            if main_mode(c) == SizeMode::Fill {
                fills += 1;
            } else {
                fixed += if row { s.0 } else { s.1 };
            }
        }
        let gaps = n.gap * kids.len().saturating_sub(1) as f64;
        let share = fill_share(inner_main, fixed, gaps, fills);
        // Justify only applies when no child fills the main axis.
        let shift = if fills == 0 {
            spread(n.justify, inner_main - fixed - gaps, kids.len())
        } else {
            spread(Default::default(), 0.0, kids.len())
        };
        let mut pos = if row { r.x } else { r.y } + n.padding + shift.offset;
        for (c, first) in items {
            // Stage 2: a child that fills the main axis is sized again with its final share.
            let s = if main_mode(c) == SizeMode::Fill {
                let cross = (cross_mode(c) == SizeMode::Fill).then_some(inner_cross);
                if row {
                    self.size_of(c, Some(share), cross)
                } else {
                    self.size_of(c, cross, Some(share))
                }
            } else {
                first
            };
            let (cw, ch) = s;
            let cross_size = if row { ch } else { cw };
            let cross_off = match n.align {
                Align::Center => (inner_cross - cross_size) / 2.0,
                Align::End => inner_cross - cross_size,
                Align::Start => 0.0,
            };
            let cross_pos = if row { r.y } else { r.x } + n.padding + cross_off;
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
            pos += if row { cw } else { ch } + n.gap + shift.extra_gap;
        }
    }

    /// Place every root at its own position; returns the boxes in drawing order.
    pub(super) fn run(mut self) -> Vec<(usize, Rect)> {
        for i in self.roots() {
            let (w, h) = self.size_of(i, None, None);
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
