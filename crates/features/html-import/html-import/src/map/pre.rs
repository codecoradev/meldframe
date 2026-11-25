//! What the mapping needs to know of every element before it builds: whether it is shown, what it paints, whether it
//! is part of a line of text, and whether any text is under it. Computed once for the whole tree, in two passes over
//! the elements (a parent comes before its children), so no element is looked at again and again by its ancestors.

use super::paint::{Paint, decorates, paint_of};
use crate::css::{Style, Styles};
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
        if !p.hidden[i] {
            p.paint[i] = paint_of(st, report);
        }
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
            && !decorates(&p.paint[i])
            && box_free(st)
            && !positioned(st)
            && st.get("visibility").is_none_or(|v| v.trim() != "hidden");
    }
    p
}

/// The white space that collapses: not the no-break space.
pub(crate) fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}')
}
