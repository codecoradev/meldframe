//! The elements that are not a plain container: images and what the model cannot hold, which are placeholders, and the
//! controls, which are frames with the text they show (`docs/import-html.md` section 3.2).

use super::build::Builder;
use super::item::{Item, Notes, blank};
use super::layout::{self, Ctx, Flow};
use super::text::Run;
use html_import_tree::{ElementId, EntryKind};
use meldframe_protocol::{LayoutMode, NodeKind, SizeMode};

/// An input that has no node: the browser draws it as a control with no text.
fn no_node(tag: &str, kind: &str) -> bool {
    tag == "option"
        || (tag == "input"
            && matches!(
                kind,
                "checkbox" | "radio" | "range" | "file" | "color" | "image"
            ))
}

impl Builder<'_> {
    /// The element `e`, which is a box (not part of a line of text), as items: none, one, or what a plain wrapper
    /// holds.
    pub fn element(&mut self, e: ElementId, ctx: &Ctx, out: &mut Vec<Item>) {
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
                let item = self.media(e, ctx);
                out.push(item);
            }
            "input" | "textarea" | "select" | "button" => {
                let item = self.field(e, ctx);
                out.push(item);
            }
            _ => self.generic(e, st, ctx, out),
        }
    }

    /// A placeholder `rect` for what the model cannot hold: the element keeps the room it took. Nothing is fetched.
    fn media(&mut self, e: ElementId, ctx: &Ctx) -> Item {
        let el = self.tree.element(e);
        let st = self.styles.get(e);
        let mut node = blank(NodeKind::Rect);
        let mut notes = Notes::default();
        self.boxed(&mut node, st, ctx, &mut notes);
        self.sizes(&mut node, st, ctx, false, (0.0, 0.0), &mut notes);
        // An attribute is a size the page gave when the style gives none; with neither the model's default stays.
        for (horizontal, attr) in [(true, "width"), (false, "height")] {
            let (size, mode) = if horizontal {
                (&mut node.w, &mut node.sw)
            } else {
                (&mut node.h, &mut node.sh)
            };
            if *mode == SizeMode::Fixed && *size == 0.0 {
                *size = el
                    .attr(attr)
                    .and_then(|v| v.trim().parse::<f64>().ok())
                    .filter(|v| (0.0..=layout::MAX_SIZE).contains(v))
                    .unwrap_or_else(|| {
                        notes.approximate(attr, "not given", "100px");
                        100.0
                    });
            }
            if *mode == SizeMode::Hug {
                *mode = SizeMode::Fixed;
            }
        }
        let mut item = self.make(e, node, Vec::new(), notes, false);
        if el.tag == "img" {
            item.src = self.pre.paint[e.index()]
                .assets
                .first()
                .map(|a| a.src.clone());
            self.report
                .count("image_src", usize::from(item.src.is_some()));
        } else {
            item.notes.refuse("element", &el.tag, "refused_content");
        }
        item
    }

    /// A control: a frame with the text it shows, as its `value` or its `placeholder`, or what is inside a button.
    fn field(&mut self, e: ElementId, ctx: &Ctx) -> Item {
        let el = self.tree.element(e);
        let st = self.styles.get(e);
        let inner = Ctx {
            flow: Flow::Row,
            stretch: false,
            sw: SizeMode::Hug,
            sh: SizeMode::Hug,
            px: (None, None),
        };
        let mut notes = std::mem::take(&mut self.pending);
        let mut kids = Vec::new();
        if matches!(el.tag.as_str(), "button" | "textarea") {
            self.content(e, &inner, &mut kids);
        } else if el.tag == "input" {
            let shown = el
                .attr("value")
                .filter(|v| !v.is_empty())
                .or_else(|| el.attr("placeholder"))
                .unwrap_or("");
            let mut run = Run::of(shown);
            self.flush(e, &inner, &mut run, &mut kids);
        }
        let mut node = blank(NodeKind::Frame);
        notes.approximate("element", &el.tag, "a frame with the text it shows");
        node.layout = LayoutMode::Row;
        node.align = meldframe_protocol::Align::Center;
        self.boxed(&mut node, st, ctx, &mut notes);
        self.sizes(&mut node, st, ctx, false, (0.0, 0.0), &mut notes);
        self.make(e, node, kids, notes, false)
    }
}
