//! The page: the `body` (or the top-level elements of a fragment) as the root frame, or as the one box in it.

use super::items::drop_blank;
use super::names;
use super::node::{Spec, blank, flow, padding, parent_of, set_padding};
use super::notes::Notes;
use super::size::Axis;
use super::text::Ws;
use super::walk::{Walk, plain};
use crate::css::{Report, Style};
use crate::html::Child;
use crate::paint::{self, Ctx};
use meldframe_protocol::{NodeKind, SizeMode};

impl Walk<'_> {
    fn title(&self) -> Option<String> {
        let t = self
            .tree
            .ids()
            .find(|i| self.tree.element(*i).tag == "title")?;
        let text: String = self
            .tree
            .element(t)
            .children
            .iter()
            .filter_map(|c| match c {
                Child::Text(t) => Some(t.as_str()),
                Child::Element(_) => None,
            })
            .collect();
        let name = names::tidy(&text, names::MAX_NAME);
        (!name.is_empty()).then_some(name)
    }

    /// The page: the `body` (or the top-level elements of a fragment) as the root frame, or the one box in it.
    pub fn page(&mut self) -> Option<Spec> {
        let tree = self.tree;
        let top = &tree.root().children;
        let element = |cs: &[Child], tag: &str| {
            cs.iter().find_map(|c| match c {
                Child::Element(e) if tree.element(*e).tag == tag => Some(*e),
                _ => None,
            })
        };
        let html = element(top, "html");
        let scope = html.map_or(top.as_slice(), |h| tree.element(h).children.as_slice());
        let body = element(scope, "body");
        let virtual_style = self
            .styles
            .body
            .as_ref()
            .or(self.styles.html.as_ref())
            .cloned()
            .unwrap_or_else(Style::root);
        let (cs, children, at): (Style, &[Child], usize) = match (body, html) {
            (Some(b), _) => (
                self.styles.get(b).clone(),
                &tree.element(b).children,
                tree.element(b).at,
            ),
            (None, Some(h)) => (
                self.styles.get(h).clone(),
                &tree.element(h).children,
                tree.element(h).at,
            ),
            (None, None) => (virtual_style, top.as_slice(), 0),
        };
        // The page is a box like any other: its `display` decides how its children are laid out, and what it declares
        // and the model cannot hold is listed on the root frame (or on the one frame that is the root).
        let mut local = Notes::default();
        let exported = body.is_some_and(|b| tree.element(b).attr("data-node").is_some());
        let root_w = cs
            .len("width")
            .filter(|l| l.px >= 0.0)
            .map_or(self.opts.width, |l| l.px);
        let fl = flow(&cs, &mut local, false, false);
        self.leftovers(&cs, exported, &mut local);
        let mut items = Vec::new();
        let mut listed = false;
        self.collect(children, &cs, &mut items, &mut local, &mut listed);
        drop_blank(&mut items, Ws::of(&cs));
        let fixed = |px: f64| Axis {
            mode: SizeMode::Fixed,
            px: Some(px),
        };
        let hug = Axis {
            mode: SizeMode::Hug,
            px: None,
        };
        let p0 = parent_of(&fl, fixed(root_w), hug, Some(root_w), None);
        let mut kids = self.children(items, &cs, &p0, "body");
        if kids.is_empty() {
            return None;
        }
        // One frame on a page that has no room of its own (the margin of the body is the browser's 8 px unless the
        // page says another) and says nothing else is the root: the same rule for one frame and for several, which are
        // inside a root frame that holds the margin as its padding.
        if kids.len() == 1 && kids[0].node.kind == NodeKind::Frame && plain(&cs) {
            let mut root = kids.remove(0);
            let node = &mut root.node;
            if node.sw == SizeMode::Fill {
                node.sw = SizeMode::Fixed;
                node.w = root_w;
            }
            if node.sh == SizeMode::Fill {
                node.sh = SizeMode::Hug;
            }
            node.absolute = false;
            let margin = [node.mar_t, node.mar_r, node.mar_b, node.mar_l];
            (node.mar_t, node.mar_r, node.mar_b, node.mar_l) = (0.0, 0.0, 0.0, 0.0);
            if margin.iter().any(|m| *m != 0.0) {
                root.notes.change(
                    "margin",
                    &format!(
                        "{}px {}px {}px {}px",
                        margin[0], margin[1], margin[2], margin[3]
                    ),
                    "0: the root frame sits at the place it is given",
                );
            }
            root.notes.merge(local);
            if let Some(name) = self.opts.name.as_deref() {
                root.node.name = names::tidy(name, names::MAX_NAME);
            } else if let Some(t) = self.title().filter(|_| !root.named) {
                root.node.name = t;
            }
            return Some(root);
        }
        let ctx = Ctx {
            width: Some(root_w),
            height: None,
            has_children: true,
        };
        let paint = {
            let mut r = Report::default();
            let p = paint::read(&cs, &ctx, at, &mut r);
            self.absorb(r, &mut local);
            p
        };
        let mut node = blank(NodeKind::Frame);
        paint.apply(&mut node);
        let pad = page_padding(&cs, &mut local, paint.border_room);
        set_padding(&mut node, pad);
        node.layout = fl.layout;
        node.gap = fl.gap;
        node.align = fl.align;
        node.justify = fl.justify;
        node.sw = SizeMode::Fixed;
        node.w = root_w;
        node.sh = SizeMode::Hug;
        node.name = match self.opts.name.as_deref() {
            Some(n) => names::tidy(n, names::MAX_NAME),
            None => self.title().unwrap_or_default(),
        };
        if node.name.is_empty() {
            node.name = "Imported HTML".into();
        }
        Some(Spec {
            node,
            source: "body".into(),
            notes: local,
            assets: paint
                .assets
                .iter()
                .cloned()
                .map(|a| ("background", a))
                .collect(),
            kids,
            named: true,
        })
    }
}

/// The margin and the padding of the page, on each side, in pixels: the root frame holds both.
fn page_padding(s: &Style, n: &mut Notes, room: f64) -> [f64; 4] {
    let pad = padding(s, n, room);
    let mut m = [0.0; 4];
    for (i, side) in ["top", "right", "bottom", "left"].iter().enumerate() {
        let prop = format!("margin-{side}");
        if let Some(l) = s.len(&prop).filter(|l| l.px >= 0.0) {
            m[i] = l.px;
        }
    }
    [pad[0] + m[0], pad[1] + m[1], pad[2] + m[2], pad[3] + m[3]]
}
