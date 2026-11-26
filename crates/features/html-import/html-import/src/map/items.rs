//! What an element holds, in order: runs of text and boxes. The walk over the children of an element, which passes
//! the content of inline elements and of `display: contents` up to the element they are in.

use super::names;
use super::node::display;
use super::notes::Notes;
use super::text::Piece;
use super::walk::Walk;
use crate::css::Style;
use crate::html::{Child, Element, ElementId};

/// What an element holds, in order: a run of text (pieces of text and line breaks that flow together) or a box.
#[derive(Debug)]
pub(super) enum Item {
    Run(Vec<Piece>),
    Box(ElementId),
}

/// Elements that are never drawn: no node and nothing worth a line in the report.
const NOT_DRAWN: &[&str] = &[
    "head", "script", "style", "link", "meta", "title", "noscript", "template", "base", "area",
    "param", "source", "track", "col", "colgroup",
];

/// How an element takes part in the flow of its parent.
pub(super) enum Role {
    /// Nothing is drawn for it, and nothing is listed.
    Skip,
    /// `display: none` or `hidden`: no node, a line under `skipped`.
    Hidden,
    /// Its content goes to the element it is in (`display: contents`, a table section).
    Through,
    /// Part of a line of text.
    Inline,
    /// A node.
    Box,
}

pub(super) fn role(el: &Element, s: &Style) -> Role {
    let d = display(s);
    if d == "none" {
        return if NOT_DRAWN.contains(&el.tag.as_str()) {
            Role::Skip
        } else {
            Role::Hidden
        };
    }
    let absolute = s
        .get("position")
        .is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "absolute" | "fixed"));
    match d.as_str() {
        "contents" | "table-header-group" | "table-row-group" | "table-footer-group" => {
            Role::Through
        }
        "table-column" | "table-column-group" => Role::Skip,
        "" | "inline" if absolute => Role::Box,
        "" | "inline" => match el.tag.as_str() {
            "img" | "svg" | "canvas" | "video" | "audio" | "iframe" | "object" | "embed"
            | "input" | "textarea" | "select" | "button" | "option" | "optgroup" => Role::Box,
            _ => Role::Inline,
        },
        _ => Role::Box,
    }
}

impl Walk<'_> {
    /// Put what `children` hold into `items`, with the style of the box they are in for comparing inline styles.
    pub(super) fn collect(
        &mut self,
        children: &[Child],
        base: &Style,
        items: &mut Vec<Item>,
        n: &mut Notes,
        listed: &mut bool,
    ) {
        let tree = self.tree;
        for c in children {
            match c {
                Child::Text(t) => push(items, Piece::Text(t.clone())),
                Child::Element(e) => {
                    let el = tree.element(*e);
                    let s = self.styles.get(*e);
                    match role(el, s) {
                        Role::Skip => {}
                        Role::Hidden => self.skip(names::source(el, *e), "display_none"),
                        Role::Through => self.collect(&el.children, base, items, n, listed),
                        Role::Box => items.push(Item::Box(*e)),
                        Role::Inline if el.tag == "br" => push(items, Piece::Br),
                        Role::Inline => {
                            if !*listed {
                                *listed = differs(base, s, n);
                            }
                            self.collect(&el.children, base, items, n, listed);
                        }
                    }
                }
            }
        }
    }

    pub(super) fn items_of(&mut self, el: &Element, s: &Style, n: &mut Notes) -> Vec<Item> {
        let mut items = Vec::new();
        let mut listed = false;
        self.collect(&el.children, s, &mut items, n, &mut listed);
        items
    }

    pub(super) fn is_absolute(&self, id: ElementId) -> bool {
        self.styles
            .get(id)
            .get("position")
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("absolute"))
    }

    /// The text a control shows, as one run: `None` when the control has no model equivalent.
    pub(super) fn control_items(
        &mut self,
        el: &Element,
        s: &Style,
        n: &mut Notes,
    ) -> Option<Vec<Item>> {
        let shown = |t: &str| {
            let t = t.trim();
            (!t.is_empty()).then(|| vec![Item::Run(vec![Piece::Text(t.to_owned())])])
        };
        match el.tag.as_str() {
            "input" => {
                let kind = el
                    .attr("type")
                    .unwrap_or("text")
                    .trim()
                    .to_ascii_lowercase();
                match kind.as_str() {
                    "checkbox" | "radio" | "range" | "file" | "color" | "image" => None,
                    "submit" | "button" | "reset" => {
                        Some(shown(el.attr("value").unwrap_or("")).unwrap_or_default())
                    }
                    _ => {
                        let v = el.attr("value").filter(|v| !v.trim().is_empty());
                        let text = v.or_else(|| el.attr("placeholder")).unwrap_or("");
                        Some(shown(text).unwrap_or_default())
                    }
                }
            }
            "select" => {
                let options: Vec<&Element> = el
                    .children
                    .iter()
                    .filter_map(|c| match c {
                        Child::Element(e) => Some(self.tree.element(*e)),
                        Child::Text(_) => None,
                    })
                    .filter(|o| o.tag == "option")
                    .collect();
                let pick = options
                    .iter()
                    .find(|o| o.attr("selected").is_some())
                    .or(options.first());
                let text: String = pick
                    .map(|o| {
                        o.children
                            .iter()
                            .filter_map(|c| match c {
                                Child::Text(t) => Some(t.as_str()),
                                Child::Element(_) => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                n.change("select", "the control", "the text of the chosen option");
                Some(shown(&text).unwrap_or_default())
            }
            _ => Some(self.items_of(el, s, n)),
        }
    }
}

/// Push a piece onto the last run, or start one.
fn push(items: &mut Vec<Item>, piece: Piece) {
    if let Some(Item::Run(run)) = items.last_mut() {
        run.push(piece);
    } else {
        items.push(Item::Run(vec![piece]));
    }
}

/// The first of color, size and weight that an inline element has other than its box: the model has one style for a
/// text, so the text takes the box's and this is listed. `true` when something was listed.
fn differs(base: &Style, inline: &Style, n: &mut Notes) -> bool {
    if inline.font_weight != base.font_weight {
        n.change(
            "font-weight",
            &format!("{} inside the text", inline.font_weight),
            &format!("{} for the whole text", base.font_weight),
        );
    } else if inline.font_size != base.font_size {
        n.change(
            "font-size",
            &format!("{}px inside the text", inline.font_size),
            &format!("{}px for the whole text", base.font_size),
        );
    } else if inline.color != base.color {
        n.change(
            "color",
            &format!("{} inside the text", inline.color),
            &format!("{} for the whole text", base.color),
        );
    } else {
        return false;
    }
    true
}
