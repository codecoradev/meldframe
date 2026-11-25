//! The structure: which elements make nodes, of which kind, and in what order. Every element is looked at once, in
//! the order of the text. The rules are the rows of `docs/import-html.md` section 3.2.

use super::item::{Item, Notes, blank};
use super::names::{MAX_SOURCE, name_of, source_of};
use super::paint::{decorates, put};
use super::plain::plain;
use super::pre::{NON_VISUAL, Pre, display, is_space, zero};
use super::text::{Piece, collapse};
use crate::css::{Style, Styles};
use html_import_tree::{Child, ElementId, EntryKind, Report, Tree, clean};
use meldframe_protocol::{LayoutMode, Node, NodeKind};

pub(crate) struct Builder<'a> {
    pub tree: &'a Tree,
    pub styles: &'a Styles,
    pub pre: &'a Pre,
    pub report: &'a mut Report,
    /// What a wrapper that went had to say: it goes to the node that takes its place.
    pub pending: Notes,
}

/// An input that has no node: the browser draws it as a control with no text.
fn no_node(tag: &str, kind: &str) -> bool {
    tag == "option"
        || (tag == "input"
            && matches!(
                kind,
                "checkbox" | "radio" | "range" | "file" | "color" | "image"
            ))
}

impl<'a> Builder<'a> {
    /// The items for the content of `el`, in the order of the text.
    pub fn content(&mut self, el: ElementId, out: &mut Vec<Item>) {
        let mut run = Vec::new();
        self.walk(el, el, &mut run, out);
        self.flush(el, &mut run, out);
    }

    fn walk(
        &mut self,
        owner: ElementId,
        el: ElementId,
        run: &mut Vec<Piece<'a>>,
        out: &mut Vec<Item>,
    ) {
        let tree = self.tree;
        for c in &tree.element(el).children {
            self.report.work += 1;
            match c {
                Child::Text(t) => run.push(Piece::Text(t)),
                Child::Element(e) => self.child(owner, *e, run, out),
            }
        }
    }

    fn child(
        &mut self,
        owner: ElementId,
        e: ElementId,
        run: &mut Vec<Piece<'a>>,
        out: &mut Vec<Item>,
    ) {
        let el = self.tree.element(e);
        if self.pre.hidden[e.index()] {
            self.skipped(e);
        } else if el.tag == "br" {
            run.push(Piece::Br);
        } else if display(self.styles.get(e)) == "contents" {
            self.report.count("display_contents", 1);
            self.walk(owner, e, run, out);
        } else if self.pre.inline[e.index()] {
            self.gather(e, run);
        } else {
            self.flush(owner, run, out);
            self.element(e, out);
        }
    }

    /// The text of an inline element goes into the line it is in.
    fn gather(&mut self, e: ElementId, run: &mut Vec<Piece<'a>>) {
        let tree = self.tree;
        for c in &tree.element(e).children {
            self.report.work += 1;
            match c {
                Child::Text(t) => run.push(Piece::Text(t)),
                Child::Element(k) if self.pre.hidden[k.index()] => self.skipped(*k),
                Child::Element(k) if tree.element(*k).tag == "br" => run.push(Piece::Br),
                Child::Element(k) => self.gather(*k, run),
            }
        }
    }

    /// An element that makes no node because it is not shown. What the browser's defaults hide (`head`, `script`)
    /// is not content and is only counted.
    fn skipped(&mut self, e: ElementId) {
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

    /// The anonymous text a line of text makes in `owner`.
    fn flush(&mut self, owner: ElementId, run: &mut Vec<Piece<'a>>, out: &mut Vec<Item>) {
        let text = collapse(run);
        run.clear();
        if text.is_empty() {
            return;
        }
        let el = self.tree.element(owner);
        let mut node = blank(NodeKind::Text);
        node.name = clean(&format!("{} text", name_of(el)), 256);
        node.text = text;
        let st = self.styles.get(owner);
        node.fill = meldframe_protocol::Bind::Lit(st.color.clone());
        out.push(Item {
            node,
            kids: Vec::new(),
            source: source_of(el),
            at: el.at,
            notes: Notes::default(),
            src: None,
            named: false,
        });
    }

    /// How many nodes the content of `e` will make: boxes, and whether there is a line of text.
    fn shape(&self, e: ElementId) -> (usize, bool) {
        let (mut boxes, mut line) = (0, false);
        for c in &self.tree.element(e).children {
            match c {
                Child::Text(t) => line |= t.chars().any(|c| !is_space(c)),
                Child::Element(k) => {
                    let i = k.index();
                    if self.pre.hidden[i] || self.tree.element(*k).tag == "br" {
                    } else if display(self.styles.get(*k)) == "contents" {
                        let (b, l) = self.shape(*k);
                        (boxes, line) = (boxes + b, line | l);
                    } else if self.pre.inline[i] {
                        line |= self.pre.text[i];
                    } else {
                        boxes += 1;
                    }
                }
            }
        }
        (boxes, line)
    }

    fn item(&mut self, e: ElementId, mut node: Node, kids: Vec<Item>) -> Item {
        let el = self.tree.element(e);
        node.name = name_of(el);
        put(&mut node, &self.pre.paint[e.index()]);
        Item {
            node,
            kids,
            source: source_of(el),
            at: el.at,
            notes: std::mem::take(&mut self.pending),
            src: None,
            named: el
                .attr("data-name")
                .is_some_and(|n| !clean(n, 256).trim().is_empty()),
        }
    }

    /// The element `e`, which is a box (not part of a line of text), as items: none, one, or what a plain wrapper
    /// holds.
    pub fn element(&mut self, e: ElementId, out: &mut Vec<Item>) {
        let el = self.tree.element(e);
        let st = self.styles.get(e);
        let kind = el.attr("type").unwrap_or("text").to_ascii_lowercase();
        match el.tag.as_str() {
            _ if no_node(&el.tag, &kind) => {
                let msg = format!("<{}> has no equivalent in the model: no node", el.tag);
                self.report
                    .note(EntryKind::Refused, "element_skipped", &el.tag, msg, el.at);
            }
            "img" | "svg" | "canvas" | "video" | "audio" | "iframe" | "object" | "embed" => {
                let item = self.media(e);
                out.push(item);
            }
            "input" | "textarea" | "select" | "button" => {
                let item = self.field(e);
                out.push(item);
            }
            _ => self.generic(e, st, out),
        }
    }

    /// A placeholder `rect` for what the model cannot hold: the element keeps the room it took. Nothing is fetched.
    fn media(&mut self, e: ElementId) -> Item {
        let el = self.tree.element(e);
        let mut item = self.item(e, blank(NodeKind::Rect), Vec::new());
        if el.tag == "img" {
            item.src = el
                .attr("src")
                .filter(|s| !s.is_empty())
                .map(|s| clean(s, MAX_SOURCE));
            self.report.count("image_src", item.src.is_some() as usize);
        } else {
            item.notes.refuse("element", &el.tag, "refused_content");
        }
        item
    }

    /// A control: a frame with the text it shows, as its `value` or its `placeholder`, or what is inside a button.
    fn field(&mut self, e: ElementId) -> Item {
        let el = self.tree.element(e);
        let mut kids = Vec::new();
        if matches!(el.tag.as_str(), "button" | "textarea") {
            self.content(e, &mut kids);
        } else if el.tag == "input" {
            let shown = el
                .attr("value")
                .filter(|v| !v.is_empty())
                .or_else(|| el.attr("placeholder"))
                .unwrap_or("");
            let mut run = vec![Piece::Text(shown)];
            self.flush(e, &mut run, &mut kids);
        }
        let mut node = blank(NodeKind::Frame);
        node.layout = LayoutMode::Column;
        // The browser draws these itself; what is kept is the box and the text it shows.
        self.pending
            .approximate("element", &el.tag, "a frame with the text it shows");
        self.item(e, node, kids)
    }

    fn generic(&mut self, e: ElementId, st: &Style, out: &mut Vec<Item>) {
        let el = self.tree.element(e);
        let paint = &self.pre.paint[e.index()];
        let (boxes, line) = self.shape(e);
        if plain(el, st, paint) && boxes + usize::from(line) == 1 {
            let msg = format!(
                "<{}> adds nothing to the layout: no node, its content takes its place",
                el.tag
            );
            self.report
                .note(EntryKind::Listed, "wrapper_collapsed", &el.tag, msg, el.at);
            self.content(e, out);
            return;
        }
        if boxes == 0 && line && self.as_text(e, st) {
            let mut pieces = Vec::new();
            self.gather(e, &mut pieces);
            let mut node = blank(NodeKind::Text);
            node.text = collapse(&pieces);
            node.fill = meldframe_protocol::Bind::Lit(st.color.clone());
            let item = self.item(e, node, Vec::new());
            out.push(item);
            return;
        }
        let mut kids = Vec::new();
        self.content(e, &mut kids);
        let childless = kids.is_empty();
        let sized = st.len("width").is_some() && st.len("height").is_some();
        let kind = if childless && self.pre.paint[e.index()].ellipse && sized {
            NodeKind::Ellipse
        } else if childless
            && sized
            && display(st) != "flex"
            && ["top", "right", "bottom", "left"]
                .iter()
                .all(|s| zero(st, &format!("padding-{s}")))
        {
            NodeKind::Rect
        } else {
            NodeKind::Frame
        };
        let mut node = blank(kind);
        if kind == NodeKind::Frame {
            node.layout = LayoutMode::Column;
        }
        let item = self.item(e, node, kids);
        out.push(item);
    }

    /// Does an element that holds only a line of text become that text (and not a frame around it)? Not when it
    /// paints a box or has padding or its own layout.
    fn as_text(&self, e: ElementId, st: &Style) -> bool {
        matches!(
            display(st).as_str(),
            "block" | "inline" | "inline-block" | "list-item" | "flow-root"
        ) && !decorates(&self.pre.paint[e.index()])
            && ["top", "right", "bottom", "left"]
                .iter()
                .all(|s| zero(st, &format!("padding-{s}")))
    }
}
