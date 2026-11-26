//! The name of a node and the label of the element it came from. Both are text the importer makes up out of the
//! document, so both are made safe to show (rule U9), once, here; the text of a node is not touched.

use crate::html::{Element, ElementId};
use meldframe_text::{Profile, sanitize, sanitize_clip};

/// The longest name of a node (`meldframe-check` `Limits::max_name`).
pub(super) const MAX_NAME: usize = 256;

/// A name made safe to show (rule U9): the display profile of `meldframe-text` (a joiner stays between emoji, every
/// other hidden character goes), every run of white space one space, the ends trimmed, cut to `max` characters.
/// A name that is only hidden or white space comes back empty. The text of a node is not made by this.
pub(super) fn tidy(s: &str, max: usize) -> String {
    let spaced = s.replace(['\t', '\n', '\r', '\u{0B}', '\u{0C}'], " ");
    let safe = sanitize(&spaced, Profile::Display);
    let folded = safe.split_whitespace().collect::<Vec<_>>().join(" ");
    sanitize_clip(&folded, Profile::Display, max, usize::MAX)
        .trim_end()
        .to_owned()
}

/// The role of an element as a name a designer would give the layer.
fn role(tag: &str) -> String {
    let r = match tag {
        "body" | "html" => "Page",
        "header" => "Header",
        "footer" => "Footer",
        "nav" => "Navigation",
        "main" => "Main",
        "aside" => "Sidebar",
        "section" => "Section",
        "article" => "Article",
        "form" => "Form",
        "fieldset" => "Fieldset",
        "figure" => "Figure",
        "figcaption" => "Caption",
        "ul" | "ol" | "dl" => "List",
        "li" | "dt" | "dd" => "List item",
        "table" => "Table",
        "tr" => "Table row",
        "td" | "th" => "Table cell",
        "button" => "Button",
        "img" => "Image",
        "input" | "textarea" | "select" => "Input",
        "svg" | "canvas" | "video" | "audio" | "iframe" | "object" | "embed" => "Embedded content",
        "blockquote" => "Quote",
        "p" => "Paragraph",
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => "Heading",
        "div" => "Container",
        "span" | "a" | "label" | "strong" | "b" | "em" | "i" | "small" | "code" | "pre" => "Text",
        _ => "",
    };
    if !r.is_empty() {
        return r.to_owned();
    }
    let mut c = tag.chars();
    c.next().map_or_else(
        || "Container".to_owned(),
        |f| f.to_uppercase().chain(c).collect(),
    )
}

/// The first class of the element, or "".
fn first_class(el: &Element) -> &str {
    el.attr("class")
        .and_then(|c| c.split_ascii_whitespace().next())
        .unwrap_or("")
}

/// The name an author gave: `data-name`, else `id`, else the first class. `None` when the element has none that
/// leaves anything once it is made safe.
pub(super) fn authored(el: &Element) -> Option<String> {
    [el.attr("data-name"), el.attr("id"), Some(first_class(el))]
        .into_iter()
        .flatten()
        .map(|v| tidy(v, MAX_NAME))
        .find(|n| !n.is_empty())
}

/// The name of a box: what the author gave, else the role of the element.
pub(super) fn of_box(el: &Element) -> String {
    authored(el).unwrap_or_else(|| tidy(&role(&el.tag), MAX_NAME))
}

/// The name of a text: what the author gave, else the first words of the text, else the role.
pub(super) fn of_text(el: Option<&Element>, text: &str) -> String {
    if let Some(n) = el.and_then(authored) {
        return n;
    }
    let first = tidy(text, 40);
    if first.is_empty() {
        el.map_or_else(|| "Text".to_owned(), of_box)
    } else {
        first
    }
}

/// `tag.class#n`: the tag, the first class and the place of the element in the text.
pub(super) fn source(el: &Element, id: ElementId) -> String {
    let class = first_class(el);
    let class = if class.is_empty() {
        String::new()
    } else {
        format!(".{class}")
    };
    sanitize_clip(
        &format!("{}{class}#{}", el.tag, id.index()),
        Profile::Strict,
        120,
        usize::MAX,
    )
}
