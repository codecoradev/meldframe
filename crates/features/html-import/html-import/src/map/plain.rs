//! Is an element a plain wrapper: one that adds nothing to the layout, so that it can go and its single child take
//! its place (`docs/import-html.md` section 3.2, the wrapper row).

use super::paint::{Paint, is_blank};
use super::pre::display;
use crate::css::Style;
use html_import_tree::Element;

/// The properties that change a box. A wrapper that has one of them set to something other than its initial value
/// is not plain.
const BOX_PROPS: &[&str] = &[
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "width",
    "height",
    "min-width",
    "min-height",
    "max-width",
    "max-height",
    "left",
    "top",
    "right",
    "bottom",
    "overflow-x",
    "overflow-y",
    "float",
    "clear",
    "transform",
    "rotate",
    "scale",
    "translate",
    "z-index",
    "flex-grow",
    "flex-basis",
    "align-self",
    "order",
    "aspect-ratio",
    "visibility",
];

fn initial(v: &str) -> bool {
    matches!(
        v.trim(),
        "0" | "0px" | "auto" | "none" | "visible" | "normal" | "static" | "relative"
    )
}

/// A `display: block` element with no box property, nothing painted, no name of its own and no `data-name`.
pub(crate) fn plain(el: &Element, st: &Style, paint: &Paint) -> bool {
    display(st) == "block"
        && is_blank(paint)
        && el.attr("data-name").is_none()
        && st.get("position").is_none_or(initial)
        && BOX_PROPS
            .iter()
            .all(|p| st.get(p).is_none_or(initial) || st.len(p).is_some_and(|l| l.px == 0.0))
}
