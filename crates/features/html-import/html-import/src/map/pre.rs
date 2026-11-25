//! What the mapping needs to know of every element before it builds: whether it is shown, what it paints, whether it
//! is part of a line of text, and whether any text is under it. Computed once for the whole tree, in two passes over
//! the elements (a parent comes before its children), so no element is looked at again and again by its ancestors.

use crate::css::{Source, Style, Styles};
use crate::paint::{self, Paint};
use html_import_tree::{Child, ElementId, Report, Tree};

/// Elements that are not content: the browser's defaults hide them, and they are skipped without a word.
pub(crate) const NON_VISUAL: &[&str] = &[
    "head", "script", "style", "link", "meta", "title", "noscript", "template", "base", "area",
    "param", "source", "track", "col", "colgroup",
];

/// Elements that make a node of their own whatever their `display`, so they never join a line of text.
const OWN_BOX: &[&str] = &[
    "img", "svg", "canvas", "video", "audio", "iframe", "object", "embed", "button", "input",
    "textarea", "select", "option", "table",
];

/// The attributes the mapping reads: the names, the style and the content of an element, what an image or a control
/// shows, and what makes a cell or a dialog what it is. The model has no place for the others (`href`, `title`, `aria-*`).
const USED: &[&str] = &[
    "id",
    "class",
    "style",
    "data-name",
    "data-node",
    "src",
    "value",
    "placeholder",
    "type",
    "width",
    "height",
    "hidden",
    "open",
    "colspan",
    "rowspan",
];

pub(crate) struct Pre {
    /// Not shown: `display: none`, or inside something that is not.
    pub hidden: Vec<bool>,
    pub paint: Vec<Paint>,
    /// Part of a line of text: an inline element with no box of its own and only such content.
    pub inline: Vec<bool>,
    /// Some text that is not white space is under it, through shown elements.
    pub text: Vec<bool>,
}

/// The first word of `display`; an element with none is inline, as in CSS.
pub(crate) fn display(st: &Style) -> String {
    st.get("display")
        .and_then(|d| d.split_ascii_whitespace().next())
        .unwrap_or("inline")
        .to_ascii_lowercase()
}

/// Is `prop` absent or its initial `0`.
pub(crate) fn zero(st: &Style, prop: &str) -> bool {
    st.get(prop).is_none_or(|v| {
        let v = v.trim();
        v == "0" || st.len(prop).is_some_and(|l| l.px == 0.0)
    })
}

/// No padding and no margin on any side.
pub(crate) fn box_free(st: &Style) -> bool {
    ["top", "right", "bottom", "left"]
        .iter()
        .all(|s| zero(st, &format!("padding-{s}")) && zero(st, &format!("margin-{s}")))
}

/// Is the element itself `visibility: hidden` (not only inheriting it)?
pub(crate) fn hides(st: &Style) -> bool {
    st.props
        .get("visibility")
        .is_some_and(|p| p.source != Source::Inherited && p.value.trim() == "hidden")
}

/// Is `value` of `prop` positioned out of the flow (or the offsets of a positioned box).
pub(crate) fn positioned(st: &Style) -> bool {
    st.get("position")
        .is_some_and(|p| !matches!(p.trim(), "static" | "relative"))
}

pub(crate) fn pre(tree: &Tree, styles: &Styles, report: &mut Report) -> Pre {
    let n = tree.len() + 1;
    let mut p = Pre {
        hidden: vec![false; n],
        paint: vec![Paint::default(); n],
        inline: vec![false; n],
        text: vec![false; n],
    };
    for id in tree.ids() {
        let el = tree.element(id);
        let st = styles.get(id);
        let i = id.index();
        p.hidden[i] = p.hidden[el.parent.index()] || display(st) == "none";
        report.work += 1;
    }
    for id in tree.ids().filter(|i| !p.hidden[i.index()]) {
        let el = tree.element(id);
        let st = styles.get(id);
        let has_children = el.children.iter().any(|c| match c {
            Child::Text(t) => t.chars().any(|c| !is_space(c)),
            Child::Element(e) => !p.hidden[e.index()],
        });
        let ignored = el
            .attrs
            .iter()
            .filter(|a| !USED.contains(&a.name.as_str()))
            .count();
        report.count("attribute_ignored", ignored);
        p.paint[id.index()] = match el.tag.as_str() {
            // What the model cannot hold paints nothing of its own; an image keeps its `src` for the agent.
            "img" => Paint::placeholder(el.attr("src").filter(|s| !s.is_empty())),
            "svg" | "canvas" | "video" | "audio" | "iframe" | "object" | "embed" => {
                Paint::placeholder(None)
            }
            _ => paint::read(
                st,
                &paint::Ctx {
                    width: border_box(st, true),
                    height: border_box(st, false),
                    has_children,
                },
                el.at,
                report,
            ),
        };
    }
    let ids: Vec<ElementId> = tree.ids().collect();
    for id in ids.into_iter().rev() {
        let el = tree.element(id);
        let st = styles.get(id);
        let i = id.index();
        report.work += 1;
        let (mut inline, mut text) = (!p.hidden[i], false);
        for c in &el.children {
            match c {
                Child::Text(t) => text |= t.chars().any(|c| !is_space(c)),
                Child::Element(e) if !p.hidden[e.index()] => {
                    inline &= p.inline[e.index()] || tree.element(*e).tag == "br";
                    text |= p.text[e.index()];
                }
                Child::Element(_) => {}
            }
        }
        p.text[i] = text && !p.hidden[i];
        p.inline[i] = inline
            && display(st) == "inline"
            && !OWN_BOX.contains(&el.tag.as_str())
            && !p.paint[i].decorates()
            && box_free(st)
            && !positioned(st)
            && !hides(st);
    }
    p
}

/// The white space that collapses: not the no-break space.
pub(crate) fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}')
}

/// The authored `width` or `height` as the size of the border box (the model's size): under `content-box` the padding is
/// added to it. `None` when it is not a length.
fn border_box(st: &Style, horizontal: bool) -> Option<f64> {
    let (prop, sides) = if horizontal {
        ("width", ["padding-left", "padding-right"])
    } else {
        ("height", ["padding-top", "padding-bottom"])
    };
    let size = st.len(prop)?.px;
    if st.get("box-sizing").map(str::trim) == Some("border-box") {
        return Some(size);
    }
    Some(
        size + sides
            .iter()
            .filter_map(|s| st.len(s))
            .map(|l| l.px)
            .sum::<f64>(),
    )
}
