//! The structure: which elements make nodes, of which kind, and in what order, each with its box. Every element is
//! looked at once, in the order of the text. The rules are the rows of `docs/import-html.md` section 3.2 and 3.4.

use super::item::{Item, Notes, blank};
use super::layout::{self, Ctx};
use super::names::{name_of, source_of};
use super::plain::plain;
use super::pre::{NON_VISUAL, Pre, display, hides, is_space, zero};
use super::size::{self, Subject};
use super::text::{
    Piece, Run, collapse, first_difference, outside_contract, space, transform, typography,
};
use crate::css::{Style, Styles};
use crate::paint::Paint;
use html_import_tree::{Child, ElementId, EntryKind, Report, Tree, clean};
use meldframe_protocol::{Bind, LayoutMode, Node, NodeKind, SizeMode};

pub(crate) struct Builder<'a> {
    pub tree: &'a Tree,
    pub styles: &'a Styles,
    pub pre: &'a Pre,
    pub report: &'a mut Report,
    /// What a wrapper that went had to say: it goes to the node that takes its place.
    pub pending: Notes,
}

/// What the content of an element will make: boxes, whether there is a line of text, and how many boxes sit at
/// their own place.
struct Shape {
    boxes: usize,
    line: bool,
    absolute: usize,
}

fn white_space_nowrap(st: &Style) -> bool {
    matches!(st.get("white-space").map(str::trim), Some("nowrap" | "pre"))
}

impl<'a> Builder<'a> {
    /// The items for the content of `el`, in the order of the text, for a frame that gives its children `ctx`.
    pub fn content(&mut self, el: ElementId, ctx: &Ctx, out: &mut Vec<Item>) {
        let mut run = Run::default();
        self.walk(el, el, ctx, &mut run, out);
        self.flush(el, ctx, &mut run, out);
    }

    fn walk(
        &mut self,
        owner: ElementId,
        el: ElementId,
        ctx: &Ctx,
        run: &mut Run<'a>,
        out: &mut Vec<Item>,
    ) {
        let tree = self.tree;
        for c in &tree.element(el).children {
            self.report.work += 1;
            match c {
                Child::Text(t) => run.pieces.push(Piece::Text(t)),
                Child::Element(e) => self.child(owner, *e, ctx, run, out),
            }
        }
    }

    fn child(
        &mut self,
        owner: ElementId,
        e: ElementId,
        ctx: &Ctx,
        run: &mut Run<'a>,
        out: &mut Vec<Item>,
    ) {
        let el = self.tree.element(e);
        if self.pre.hidden[e.index()] {
            self.skipped(e);
        } else if el.tag == "br" {
            run.pieces.push(Piece::Br);
        } else if display(self.styles.get(e)) == "contents" {
            self.report.count("display_contents", 1);
            self.walk(owner, e, ctx, run, out);
        } else if self.pre.inline[e.index()] {
            self.compare(owner, e, run);
            self.gather(owner, e, run);
        } else {
            self.flush(owner, ctx, run, out);
            self.element(e, ctx, out);
        }
    }

    /// The text of an inline element goes into the line it is in.
    fn gather(&mut self, owner: ElementId, e: ElementId, run: &mut Run<'a>) {
        let tree = self.tree;
        for c in &tree.element(e).children {
            self.report.work += 1;
            match c {
                Child::Text(t) => run.pieces.push(Piece::Text(t)),
                Child::Element(k) if self.pre.hidden[k.index()] => self.skipped(*k),
                Child::Element(k) if tree.element(*k).tag == "br" => run.pieces.push(Piece::Br),
                Child::Element(k) => {
                    self.compare(owner, *k, run);
                    self.gather(owner, *k, run);
                }
            }
        }
    }

    /// The first way an inline element joined into the text of `owner` differs from it is listed once for the node.
    fn compare(&self, owner: ElementId, e: ElementId, run: &mut Run<'a>) {
        if run.diff.is_none() {
            run.diff = first_difference(self.styles.get(owner), self.styles.get(e));
        }
    }

    /// An element that makes no node because it is not shown. What the browser's defaults hide (`head`, `script`)
    /// is not content and is only counted.
    pub(super) fn skipped(&mut self, e: ElementId) {
        let el = self.tree.element(e);
        if NON_VISUAL.contains(&el.tag.as_str()) {
            self.report.count("non_visual", 1);
        } else {
            let msg = format!(
                "<{}> is not shown (display: none or hidden): no node",
                el.tag
            );
            self.report
                .note(EntryKind::Listed, "display_none", &el.tag, msg, el.at);
        }
    }

    /// A text node for a run of pieces in the style `st`: its content, typography and what inline elements changed. No
    /// node for a run with no text in it.
    fn text_node(&mut self, st: &Style, run: Run<'a>, notes: &mut Notes) -> Option<Node> {
        let how = space(st, notes);
        let text = collapse(&run.pieces, how);
        if text.is_empty() {
            return None;
        }
        let mut node = blank(NodeKind::Text);
        node.text = transform(text, st, notes);
        typography(&mut node, st, notes);
        if let Some(d) = run.diff {
            notes.approx.push(d);
        }
        Some(node)
    }

    /// The anonymous text a line of text makes in `owner`.
    pub(super) fn flush(
        &mut self,
        owner: ElementId,
        ctx: &Ctx,
        run: &mut Run<'a>,
        out: &mut Vec<Item>,
    ) {
        let run = std::mem::take(run);
        let st = self.styles.get(owner);
        let mut notes = Notes::default();
        let Some(mut node) = self.text_node(st, run, &mut notes) else {
            return;
        };
        let el = self.tree.element(owner);
        node.name = clean(&format!("{} text", name_of(el)), 256);
        // The run has no size of its own: it is auto, in the way of the frame it is in.
        let subject = Subject {
            text: true,
            horizontal: true,
            nowrap: white_space_nowrap(st),
            absolute: false,
            pinned: false,
        };
        let w = size::axis(&Style::root(), subject, ctx, &mut notes);
        let h = size::axis(
            &Style::root(),
            Subject {
                horizontal: false,
                ..subject
            },
            ctx,
            &mut notes,
        );
        (node.sw, node.sh) = (w.mode, h.mode);
        out.push(self.make(owner, node, Vec::new(), notes, true));
    }

    /// How many nodes the content of `e` will make.
    fn shape(&self, e: ElementId) -> Shape {
        let mut s = Shape {
            boxes: 0,
            line: false,
            absolute: 0,
        };
        for c in &self.tree.element(e).children {
            match c {
                Child::Text(t) => s.line |= t.chars().any(|c| !is_space(c)),
                Child::Element(k) => {
                    let i = k.index();
                    if self.pre.hidden[i] || self.tree.element(*k).tag == "br" {
                    } else if display(self.styles.get(*k)) == "contents" {
                        let inner = self.shape(*k);
                        s.boxes += inner.boxes;
                        s.line |= inner.line;
                        s.absolute += inner.absolute;
                    } else if self.pre.inline[i] {
                        s.line |= self.pre.text[i];
                    } else {
                        s.boxes += 1;
                        s.absolute += usize::from(positioned_absolute(self.styles.get(*k)));
                    }
                }
            }
        }
        s
    }

    /// An item for element `e`, with the notes of a wrapper that went before it and its own.
    pub(super) fn make(
        &mut self,
        e: ElementId,
        node: Node,
        kids: Vec<Item>,
        notes: Notes,
        anon: bool,
    ) -> Item {
        let mut node = node;
        let el = self.tree.element(e);
        if node.name.is_empty() {
            node.name = name_of(el);
        }
        let mut all = std::mem::take(&mut self.pending);
        if node.kind == NodeKind::Text && outside_contract(&node.text) {
            let msg = "the text has a tab, a URL or CJK that the browser breaks differently from the canvas".to_owned();
            self.report.note(
                EntryKind::Listed,
                "text_outside_contract",
                &node.name,
                msg,
                el.at,
            );
        }
        if !anon {
            put(&mut node, &self.pre.paint[e.index()]);
            // The box keeps its room in CSS; the model has no visibility, only opacity.
            if hides(self.styles.get(e)) {
                node.opacity = Bind::Lit(0.0);
                all.approximate("visibility", "hidden", "opacity 0");
            }
        }
        all.approx.extend(notes.approx);
        all.unsupported.extend(notes.unsupported);
        Item {
            node,
            kids,
            source: source_of(el),
            at: el.at,
            notes: all,
            src: None,
            named: !anon
                && el
                    .attr("data-name")
                    .is_some_and(|n| !clean(n, 256).trim().is_empty()),
        }
    }

    /// The box every node has: its margin, and where it sits.
    pub(super) fn boxed(&self, node: &mut Node, st: &Style, ctx: &Ctx, notes: &mut Notes) {
        let m = layout::sides(st, "margin", notes);
        [node.mar_t, node.mar_r, node.mar_b, node.mar_l] = m;
        (node.x, node.y, node.absolute) = layout::place(st, ctx, notes);
        layout::align_self(st, notes);
    }

    /// The sizes of a node: `grow` is what padding adds to a fixed size under `content-box`.
    pub(super) fn sizes(
        &self,
        node: &mut Node,
        st: &Style,
        ctx: &Ctx,
        text: bool,
        grow: (f64, f64),
        notes: &mut Notes,
    ) {
        let pinned = size::limits(st, text, notes);
        let subject = Subject {
            text,
            horizontal: true,
            nowrap: white_space_nowrap(st),
            absolute: node.absolute,
            pinned,
        };
        let w = size::axis(st, subject, ctx, notes);
        let h = size::axis(
            st,
            Subject {
                horizontal: false,
                ..subject
            },
            ctx,
            notes,
        );
        let border_box = st.get("box-sizing").map(str::trim) == Some("border-box");
        let mut grown = false;
        for (a, g, size, mode) in [
            (w, grow.0, &mut node.w, &mut node.sw),
            (h, grow.1, &mut node.h, &mut node.sh),
        ] {
            *mode = a.mode;
            if let Some(px) = a.px {
                *size = px + if border_box { 0.0 } else { g };
                grown |= !border_box && g > 0.0;
            }
        }
        if grown {
            notes.approximate("box-sizing", "content-box", "border-box");
        }
    }

    pub(super) fn generic(&mut self, e: ElementId, st: &Style, ctx: &Ctx, out: &mut Vec<Item>) {
        let el = self.tree.element(e);
        let paint = &self.pre.paint[e.index()];
        let shape = self.shape(e);
        if plain(el, st, paint) && shape.boxes + usize::from(shape.line) == 1 {
            let msg = format!(
                "<{}> adds nothing to the layout: no node, its content takes its place",
                el.tag
            );
            self.report
                .note(EntryKind::Listed, "wrapper_collapsed", &el.tag, msg, el.at);
            self.content(e, ctx, out);
            return;
        }
        let mut notes = Notes::default();
        if shape.boxes == 0 && shape.line && self.as_text(e, st) {
            let mut run = Run::default();
            self.gather(e, e, &mut run);
            let Some(mut node) = self.text_node(st, run, &mut notes) else {
                return;
            };
            self.boxed(&mut node, st, ctx, &mut notes);
            self.sizes(&mut node, st, ctx, true, (0.0, 0.0), &mut notes);
            out.push(self.make(e, node, Vec::new(), notes, false));
            return;
        }
        let free = !matches!(
            display(st).as_str(),
            "flex" | "inline-flex" | "grid" | "inline-grid"
        ) && shape.boxes > 0
            && shape.absolute == shape.boxes
            && !shape.line;
        let mut node = blank(NodeKind::Frame);
        self.boxed(&mut node, st, ctx, &mut notes);
        let c = layout::container(st, free, &mut notes);
        node.layout = c.layout;
        (node.gap, node.align, node.justify) = (c.gap, c.align, c.justify);
        node.clip = layout::clip(st, &mut notes);
        let mut pad = layout::sides(st, "padding", &mut notes);
        let room = self.pre.paint[e.index()].border_room;
        pad.iter_mut().for_each(|p| *p += room);
        layout::set_padding(&mut node, pad);
        self.sizes(
            &mut node,
            st,
            ctx,
            false,
            (pad[1] + pad[3], pad[0] + pad[2]),
            &mut notes,
        );
        let inner = Ctx {
            flow: c.flow,
            stretch: c.stretch,
            sw: node.sw,
            sh: node.sh,
            px: (
                (node.sw == SizeMode::Fixed).then_some(node.w),
                (node.sh == SizeMode::Fixed).then_some(node.h),
            ),
        };
        let mut kids = Vec::new();
        self.content(e, &inner, &mut kids);
        let sized = node.sw == SizeMode::Fixed
            && node.sh == SizeMode::Fixed
            && st.len("width").is_some()
            && st.len("height").is_some();
        let childless = kids.is_empty();
        if childless && self.pre.paint[e.index()].ellipse && sized {
            node.kind = NodeKind::Ellipse;
        } else if childless
            && sized
            && display(st) != "flex"
            && ["top", "right", "bottom", "left"]
                .iter()
                .all(|s| zero(st, &format!("padding-{s}")))
        {
            node.kind = NodeKind::Rect;
        }
        if node.kind != NodeKind::Frame {
            // A rect holds no children and ignores layout and padding.
            node.layout = LayoutMode::None;
            node.clip = false;
            node.padding = 0.0;
            [node.pad_t, node.pad_r, node.pad_b, node.pad_l] = [None; 4];
        }
        out.push(self.make(e, node, kids, notes, false));
    }

    /// Does an element that holds only a line of text become that text (and not a frame around it)? Not when it
    /// paints a box or a radius, or has padding or its own layout: a text has none of them.
    fn as_text(&self, e: ElementId, st: &Style) -> bool {
        matches!(
            display(st).as_str(),
            "block" | "inline" | "inline-block" | "list-item" | "flow-root"
        ) && !self.pre.paint[e.index()].decorates()
            && self.pre.paint[e.index()].radius == Bind::Lit(0.0)
            && ["top", "right", "bottom", "left"]
                .iter()
                .all(|s| zero(st, &format!("padding-{s}")))
            && !positioned_clip(st)
    }
}

fn positioned_absolute(st: &Style) -> bool {
    st.get("position").map(str::trim) == Some("absolute")
}

/// A text with `overflow` that clips is a frame around a text: the model's text has no clip.
fn positioned_clip(st: &Style) -> bool {
    ["overflow-x", "overflow-y"].iter().any(|p| {
        matches!(
            st.get(p).map(str::trim),
            Some("hidden" | "clip" | "auto" | "scroll")
        )
    })
}

/// Put the paint of an element on its node. A text paints no box: it takes only the opacity.
pub(crate) fn put(node: &mut Node, p: &Paint) {
    if node.kind == NodeKind::Text {
        node.opacity = p.opacity.clone();
    } else {
        p.apply(node);
    }
}
