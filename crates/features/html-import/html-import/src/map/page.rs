//! The page the elements sit in: the `body` (and the `html` above it), real or, for a fragment, the virtual ones the
//! cascade made, and the title. Their margin and padding become the padding of the root frame.

use super::item::Notes;
use super::layout::sides;
use super::pre::Pre;
use crate::css::{Style, Styles};
use crate::paint::{self, Paint};
use html_import_tree::{Child, ElementId, Report, Tree};

pub(crate) struct Page {
    /// The element whose children are the content: the `body`, else the `html`, else the root of the tree.
    pub container: ElementId,
    /// The text of the first `<title>`, collapsed.
    pub title: Option<String>,
    /// The margin and the padding of the `html` and the `body`, added, top first.
    pub pad: [f64; 4],
    /// A `width` or `height` in pixels on the `body`, else on the `html`.
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// The paint of the `body`, else of the `html`.
    pub paint: Paint,
    /// What the page said that the root cannot hold.
    pub notes: Notes,
}

impl Page {
    /// Does the page add nothing: so that a single top-level frame can be the root itself.
    pub fn is_plain(&self) -> bool {
        self.pad == [0.0; 4]
            && self.width.is_none()
            && self.height.is_none()
            && self.paint == Paint::default()
    }
}

fn child_with(tree: &Tree, of: ElementId, tag: &str) -> Option<ElementId> {
    tree.element(of).children.iter().find_map(|c| match c {
        Child::Element(e) if tree.element(*e).tag == tag => Some(*e),
        _ => None,
    })
}

pub(crate) fn find(tree: &Tree, styles: &Styles, pre: &Pre, report: &mut Report) -> Page {
    let html = child_with(tree, tree.root_id(), "html");
    let body = match html {
        Some(h) => child_with(tree, h, "body"),
        None => child_with(tree, tree.root_id(), "body"),
    };
    let mut boxes: Vec<(&Style, Paint)> = Vec::new();
    for (el, virt) in [(html, &styles.html), (body, &styles.body)] {
        match (el, virt) {
            (Some(e), _) => boxes.push((styles.get(e), pre.paint[e.index()].clone())),
            (None, Some(v)) => boxes.push((v, paint::read(v, &paint::Ctx::default(), 0, report))),
            (None, None) => {}
        }
    }
    let mut notes = Notes::default();
    let mut pad = [0.0; 4];
    let mut margin = 0.0;
    for (st, _) in &boxes {
        let m = sides(st, "margin", &mut notes);
        let p = sides(st, "padding", &mut notes);
        margin += m.iter().sum::<f64>();
        for i in 0..4 {
            pad[i] += m[i] + p[i];
        }
    }
    if margin > 0.0 {
        notes.approximate("margin", &format!("{margin}px in all"), "padding");
    }
    let size = |prop: &str, notes: &mut Notes| {
        boxes.iter().rev().find_map(|(st, _)| {
            st.get(prop)
                .and_then(|_| super::layout::size_px(st, prop, notes))
        })
    };
    let (width, height) = (size("width", &mut notes), size("height", &mut notes));
    let paint = boxes
        .iter()
        .rev()
        .map(|(_, p)| p)
        .find(|p| **p != Paint::default())
        .cloned()
        .unwrap_or_default();
    let title = tree
        .ids()
        .find(|i| tree.element(*i).tag == "title")
        .and_then(|t| {
            tree.element(t).children.iter().find_map(|c| match c {
                Child::Text(s) => Some(super::text::collapse_plain(s)),
                Child::Element(_) => None,
            })
        })
        .filter(|t| !t.is_empty());
    Page {
        container: body.or(html).unwrap_or_else(|| tree.root_id()),
        title,
        pad,
        width,
        height,
        paint,
        notes,
    }
}
