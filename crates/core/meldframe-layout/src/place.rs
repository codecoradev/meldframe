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
        // The boxes of the children, then placed in sibling order (the drawing order): the flow children get theirs
        // from the loop below, the absolute children of a layout frame and every child of a frame without layout sit at
        // their own offsets.
        let all = self.children(&n.id);
        let layout = is_layout_frame(n);
        let mut boxes: Vec<(usize, Rect)> = Vec::with_capacity(all.len());
        for &c in &all {
            if !layout || nodes[c].absolute {
                let s = self.size_of(c, None, None);
                let cn = &nodes[c];
                boxes.push((
                    c,
                    Rect {
                        x: r.x + cn.x,
                        y: r.y + cn.y,
                        w: s.0,
                        h: s.1,
                    },
                ));
            }
        }
        if !layout {
            for (c, rect) in boxes {
                self.place(c, rect);
            }
            return;
        }
        let kids = self.flow_children(&n.id);
        let row = n.layout == LayoutMode::Row;
        let pad = n.pad();
        let inner_w = r.w - pad.horizontal();
        let inner_h = r.h - pad.vertical();
        let (inner_main, inner_cross) = if row {
            (inner_w, inner_h)
        } else {
            (inner_h, inner_w)
        };
        let main_mode = |k: usize| if row { nodes[k].sw } else { nodes[k].sh };
        let cross_mode = |k: usize| if row { nodes[k].sh } else { nodes[k].sw };
        // The margins of a child along the main axis (start, end) and the cross axis (start, end).
        let main_margin = |k: usize| {
            let m = nodes[k].margin();
            if row {
                (m.left, m.right)
            } else {
                (m.top, m.bottom)
            }
        };
        let cross_margin = |k: usize| {
            let m = nodes[k].margin();
            if row {
                (m.top, m.bottom)
            } else {
                (m.left, m.right)
            }
        };

        // Stage 1: size every child with only the cross axis assigned.
        let mut items: Vec<(usize, (f64, f64))> = Vec::with_capacity(kids.len());
        for &c in &kids {
            let (cs, ce) = cross_margin(c);
            let cross = (cross_mode(c) == SizeMode::Fill).then_some(inner_cross - cs - ce);
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
            let (ms, me) = main_margin(c);
            // A margin is fixed room, for a child that fills the main axis as for one that does not.
            fixed += ms + me;
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
        let mut pos = if row { r.x + pad.left } else { r.y + pad.top } + shift.offset;
        for (c, first) in items {
            let (ms, me) = main_margin(c);
            let (cs, ce) = cross_margin(c);
            // Stage 2: a child that fills the main axis is sized again with its final share.
            let s = if main_mode(c) == SizeMode::Fill {
                let cross = (cross_mode(c) == SizeMode::Fill).then_some(inner_cross - cs - ce);
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
            // Aligned as a whole with its margins, then moved in by the start margin.
            let footprint = cross_size + cs + ce;
            let cross_off = cs
                + match n.align {
                    Align::Center => (inner_cross - footprint) / 2.0,
                    Align::End => inner_cross - footprint,
                    Align::Start => 0.0,
                };
            let cross_pos = if row { r.y + pad.top } else { r.x + pad.left } + cross_off;
            let pos_here = pos + ms;
            let rect = if row {
                Rect {
                    x: pos_here,
                    y: cross_pos,
                    w: cw,
                    h: ch,
                }
            } else {
                Rect {
                    x: cross_pos,
                    y: pos_here,
                    w: cw,
                    h: ch,
                }
            };
            boxes.push((c, rect));
            pos += ms + if row { cw } else { ch } + me + n.gap + shift.extra_gap;
        }
        // Drawing order is sibling order, whichever way a child got its box. Without an absolute child the boxes are
        // already in that order, so the common case sorts nothing.
        if all.iter().any(|&c| nodes[c].absolute) {
            let order: std::collections::HashMap<usize, usize> =
                all.iter().enumerate().map(|(k, &c)| (c, k)).collect();
            boxes.sort_by_key(|(c, _)| order[c]);
        }
        for (c, rect) in boxes {
            self.place(c, rect);
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
