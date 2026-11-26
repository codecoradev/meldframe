//! The walk over the styled tree: which elements are boxes and which are text, the runs of text between boxes, and
//! the page (the body, or the top-level elements of a fragment) the boxes go into.

use super::items::Item;
use super::leaf::{clip, common, size_text, text_node};
use super::names;
use super::node::{
    Flow, Spec, authored_padding, blank, content_box, display, fixed, flow, parent_of, place,
    set_padding, set_size,
};
use super::notes::Notes;
use super::refuse;
use super::size::{Axis, Kind, Parent, axis};
use super::text::TextStyle;
use super::{MAX_SKIPPED, Options, Skipped};
use crate::css::{EntryKind, Report, Source, Style, Styles};
use crate::html::{Element, ElementId, Tree};
use crate::paint::{self, Ctx, Paint};
use meldframe_protocol::{Align, Justify, NodeKind, SizeMode};

/// Tags whose content the browser draws itself: a box of the size given, nothing inside.
const REPLACED: &[&str] = &[
    "img", "svg", "canvas", "video", "audio", "iframe", "object", "embed",
];

/// Properties of a text that say nothing about the box.
const NEUTRAL: &[&str] = &[
    "color",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "line-height",
    "text-align",
    "text-transform",
    "white-space",
    "box-sizing",
    "letter-spacing",
    "cursor",
    "word-break",
    "list-style",
    "list-style-type",
    "list-style-position",
];

/// A declaration that changes nothing about a box: the initial value, or a property of the text only.
fn quiet(name: &str, value: &str) -> bool {
    let v = value.trim().to_ascii_lowercase();
    if NEUTRAL.contains(&name) || (name.starts_with("border-") && name.ends_with("-color")) {
        return true;
    }
    match name {
        "opacity" => v == "1",
        "position" => v == "relative" || v == "static",
        _ => matches!(
            v.as_str(),
            "0" | "0px"
                | "none"
                | "auto"
                | "visible"
                | "static"
                | "block"
                | "inline"
                | "transparent"
                | "normal"
                | "visible visible"
        ),
    }
}

/// Does the element say nothing of its own as a box: no declaration of the author that changes it? Such a wrapper
/// makes no node.
pub(super) fn plain(s: &Style) -> bool {
    s.declared().all(|(name, p)| {
        !matches!(p.source, Source::Author | Source::Inline) || quiet(name, &p.value)
    }) && no_room(s)
}

/// Does a box that holds only a run of text still do something with it? A flex box that aligns or justifies, a grid
/// (the frame lists its display) and a box that clips are a frame with a text in it: as a text alone the placement or
/// the clip would be lost with nothing said. A flex box that places nothing is the text.
fn places_text(s: &Style) -> bool {
    let d = display(s);
    let mut scratch = Notes::default();
    if d.contains("grid") {
        return true;
    }
    if d.contains("flex") {
        let f = flow(s, &mut scratch, false, false);
        if f.align != Align::Start || f.justify != Justify::Start || !scratch.lost.is_empty() {
            return true;
        }
    }
    clip(s, &mut scratch)
}

/// No margin and no padding, the browser's own (`ul`, `blockquote`, the `body`) as much as the author's: a wrapper
/// that holds room is a box of the page, and so is a margin that is not a length in pixels.
fn no_room(s: &Style) -> bool {
    [
        "margin-top",
        "margin-right",
        "margin-bottom",
        "margin-left",
        "padding-top",
        "padding-right",
        "padding-bottom",
        "padding-left",
    ]
    .iter()
    .all(|p| s.get(p).is_none() || s.len(p).is_some_and(|l| l.px == 0.0))
}

pub(super) struct Walk<'a> {
    pub(super) tree: &'a Tree,
    pub(super) styles: &'a Styles,
    pub(super) report: &'a mut Report,
    pub(super) opts: &'a Options,
    skipped: Vec<Skipped>,
    skipped_omitted: usize,
    /// The nodes made so far. Past the cap of a call the walk makes no more (`over`), since the call is refused.
    made: usize,
}

impl<'a> Walk<'a> {
    pub fn new(
        tree: &'a Tree,
        styles: &'a Styles,
        report: &'a mut Report,
        opts: &'a Options,
    ) -> Self {
        Walk {
            tree,
            styles,
            report,
            opts,
            skipped: Vec::new(),
            skipped_omitted: 0,
            made: 0,
        }
    }

    /// How many nodes were made (a lower bound once the cap of a call was passed: the walk stops there).
    pub fn made(&self) -> usize {
        self.made
    }

    pub(super) fn over(&self) -> bool {
        self.made > super::MAX_COMMANDS
    }

    pub fn into_skipped(self) -> (Vec<Skipped>, usize) {
        (self.skipped, self.skipped_omitted)
    }

    pub(super) fn skip(&mut self, source: String, code: &'static str) {
        if self.skipped.len() < MAX_SKIPPED {
            self.skipped.push(Skipped { source, code });
        } else {
            self.skipped_omitted += 1;
        }
    }

    /// Take what the paint stage said about one box (it names the box by its byte, but the box is known): refusals
    /// and bounds are lost properties, a changed value is an approximation, and the counts join the report.
    pub(super) fn absorb(&mut self, local: Report, n: &mut Notes) {
        for e in local.entries {
            let prop = e.name.as_str();
            let msg = e
                .message
                .strip_prefix(&format!("{prop}: "))
                .unwrap_or(&e.message);
            let (value, why) = msg.rsplit_once(": ").unwrap_or((msg, ""));
            match e.kind {
                EntryKind::Listed => n.change(prop, value, why),
                _ => n.lose(prop, value, e.code),
            }
        }
        for (code, count) in local.counts {
            self.report.count(code, count);
        }
        self.report.work += local.work;
    }

    pub(super) fn read_paint(
        &mut self,
        s: &Style,
        el: &Element,
        ctx: &Ctx,
        n: &mut Notes,
    ) -> Paint {
        let mut local = Report::default();
        let p = paint::read(s, ctx, el.at, &mut local);
        self.absorb(local, n);
        p
    }

    /// What the declarations of a box say that no stage carried: the notes of the style stage, the properties the
    /// mapping does not read, and generated content.
    pub(super) fn leftovers(&mut self, s: &Style, exported: bool, n: &mut Notes) {
        for note in &s.notes {
            n.lose(&note.property, &note.value, note.code);
        }
        refuse::scan(s, exported, n);
        let content = |p: &crate::css::Props| p.get("content").map(|c| c.value.trim().to_owned());
        let real = |c: Option<String>, empty_ok: bool| {
            c.is_some_and(|c| {
                !matches!(c.as_str(), "none" | "normal") && !(empty_ok && c == "\"\"")
            })
        };
        let (b, a) = (
            s.before.as_ref().and_then(content),
            s.after.as_ref().and_then(content),
        );
        if real(b, false) {
            self.report.count("generated_content", 1);
        }
        if real(a, true) {
            self.report.count("generated_content", 1);
        }
        if display(s) == "list-item" {
            self.report.count("list_marker", 1);
        }
    }

    /// One element as a node, or none (it is hidden, or only a wrapper).
    pub(super) fn map_el(&mut self, id: ElementId, p: &Parent) -> Option<Spec> {
        if self.over() {
            return None;
        }
        let spec = self.map_one(id, p);
        self.made += usize::from(spec.is_some());
        spec
    }

    fn map_one(&mut self, id: ElementId, p: &Parent) -> Option<Spec> {
        let (tree, styles) = (self.tree, self.styles);
        let (el, s) = (tree.element(id), styles.get(id));
        let source = names::source(el, id);
        self.report.work += 1;
        let tag = el.tag.as_str();
        if matches!(tag, "option" | "optgroup") {
            self.skip(source, "no_model_equivalent");
            return None;
        }
        if REPLACED.contains(&tag) {
            return Some(self.replaced(id, p, source));
        }
        let mut n = Notes::default();
        let control = matches!(tag, "input" | "textarea" | "select" | "button");
        let Some(items) = self.control_items(el, s, &mut n) else {
            self.skip(source, "no_model_equivalent");
            return None;
        };
        // A wrapper that holds one box and says nothing of its own is that box.
        if !control
            && let [Item::Box(c)] = items.as_slice()
            && plain(s)
            && names::authored(el).is_none()
            && !self.is_absolute(*c)
        {
            // What the wrapper's inline children could not carry is said on the box it collapses into.
            return self.map_one(*c, p).map(|mut spec| {
                spec.notes.merge(n);
                spec
            });
        }
        let w_px = s.len("width").filter(|l| l.px >= 0.0).map(|l| l.px);
        let h_px = s.len("height").filter(|l| l.px >= 0.0).map(|l| l.px);
        let ctx = Ctx {
            width: w_px,
            height: h_px,
            has_children: !items.is_empty(),
        };
        let paint = self.read_paint(s, el, &ctx, &mut n);
        self.leftovers(s, el.attr("data-node").is_some(), &mut n);
        let at = place(s, &mut n);
        let mut apad = authored_padding(s, &mut n);
        if control {
            apad = [0.0; 4];
        }
        let padded = apad.iter().any(|v| *v != 0.0);
        let inline = display(s).starts_with("inline");
        let kind = Kind {
            absolute: at.absolute,
            inline,
            ..Kind::default()
        };
        let named = names::authored(el).is_some();

        // A text: the element holds one run and paints nothing of its own.
        if !control
            && let [Item::Run(pieces)] = items.as_slice()
            && !paint.decorates()
            && !padded
            && !places_text(s)
        {
            let ts = TextStyle::of(s);
            let text = ts.text(pieces);
            if text.is_empty() {
                return None;
            }
            ts.report(&mut n);
            let mut node = text_node(&ts, &text);
            let aw = axis(
                s,
                &mut n,
                p,
                true,
                Kind {
                    nowrap: ts.ws.hugs(),
                    ..kind
                },
            );
            let ah = axis(s, &mut n, p, false, kind);
            size_text(&mut node, aw, ah, &ts);
            node.opacity = paint.opacity.clone();
            common(&mut node, s, p, &at, &mut n);
            node.name = names::of_text(Some(el), &text);
            return Some(Spec {
                node,
                source,
                notes: n,
                assets: paint
                    .assets
                    .iter()
                    .cloned()
                    .map(|a| ("background", a))
                    .collect(),
                kids: Vec::new(),
                named,
            });
        }

        let room = paint.border_room;
        let pad = super::node::padding(s, &mut n, room);
        let grow_box = |pad: [f64; 4]| {
            if content_box(s) {
                (pad[1] + pad[3], pad[0] + pad[2])
            } else {
                (0.0, 0.0)
            }
        };
        let aw = axis(s, &mut n, p, true, kind);
        let ah = axis(s, &mut n, p, false, kind);

        // A box with nothing in it: an ellipse, a rect, or a frame that hugs its padding.
        if items.is_empty() && !control {
            let flex = display(s).contains("flex");
            // A rect has a size that content does not decide: each axis is a pixel size or a fill. An empty box that
            // hugs on an axis is a frame (the export writes a rect and an empty frame alike).
            let decided = |a: Axis| a.mode != SizeMode::Hug;
            let sized = decided(aw) && decided(ah);
            let shape = if paint.ellipse && sized {
                Some(NodeKind::Ellipse)
            } else if !flex && sized && !padded {
                Some(NodeKind::Rect)
            } else {
                None
            };
            if paint.ellipse && shape != Some(NodeKind::Ellipse) {
                n.lose("border-radius", "50%", "refused_no_size");
            }
            if let Some(kind) = shape {
                let mut node = blank(kind);
                paint.apply(&mut node);
                common(&mut node, s, p, &at, &mut n);
                let g = if content_box(s) { 2.0 * room } else { 0.0 };
                set_size(&mut node, aw, ah, (g, g));
                node.name = names::of_box(el);
                return Some(Spec {
                    node,
                    source,
                    notes: n,
                    assets: paint
                        .assets
                        .iter()
                        .cloned()
                        .map(|a| ("background", a))
                        .collect(),
                    kids: Vec::new(),
                    named,
                });
            }
        }

        // A frame.
        let all_abs = !items.is_empty()
            && items
                .iter()
                .all(|i| matches!(i, Item::Box(c) if self.is_absolute(*c)));
        let fl: Flow = flow(s, &mut n, all_abs && !control, tag == "button");
        let mut node = blank(NodeKind::Frame);
        paint.apply(&mut node);
        set_padding(&mut node, pad);
        node.layout = fl.layout;
        node.gap = fl.gap;
        node.align = fl.align;
        node.justify = fl.justify;
        node.clip = clip(s, &mut n);
        let g = grow_box(pad);
        set_size(&mut node, aw, ah, g);
        common(&mut node, s, p, &at, &mut n);
        let child_p = parent_of(&fl, aw, ah, fixed(aw, g.0), fixed(ah, g.1));
        let kids = self.children(items, s, &child_p, &source);
        node.name = names::of_box(el);
        Some(Spec {
            node,
            source,
            notes: n,
            assets: paint
                .assets
                .iter()
                .cloned()
                .map(|a| ("background", a))
                .collect(),
            kids,
            named,
        })
    }

    pub(super) fn children(
        &mut self,
        items: Vec<Item>,
        base: &Style,
        p: &Parent,
        source: &str,
    ) -> Vec<Spec> {
        let mut out = Vec::new();
        for item in items {
            let spec = match item {
                Item::Run(pieces) => self.run_spec(&pieces, base, p, source),
                Item::Box(c) => self.map_el(c, p),
            };
            out.extend(spec);
        }
        out
    }
}
