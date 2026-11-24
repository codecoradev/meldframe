//! The element rules of the builder, as data: which elements have no content, which hold raw text, and which start
//! tags end an element that is open. This is the whole of the subset (ADR-0024): there is no other recovery than
//! these tables and the end-tag rule in `build.rs`, no formatting-element adoption, no table foster parenting and
//! no implied `html`, `head` or `body`.

/// Elements that have no content and no end tag.
pub(super) fn is_void(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

/// How the content of an element is read.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Raw {
    /// Up to the end tag, dropped with the element and never read.
    Script,
    /// Up to the end tag, kept as written: it is the CSS for the next slice.
    Style,
    /// Up to the end tag, as text with references read: no markup inside.
    Text,
}

pub(super) fn raw(tag: &str) -> Option<Raw> {
    match tag {
        "script" => Some(Raw::Script),
        "style" => Some(Raw::Style),
        "title" | "textarea" => Some(Raw::Text),
        _ => None,
    }
}

/// What a start tag ends: the nearest open element named in `closes`, looking down the open elements and giving
/// up at one named in `stops` (or past the first one with `top_only`). Closing it closes what is inside it too.
pub(super) struct Implied {
    pub closes: &'static [&'static str],
    pub stops: &'static [&'static str],
    pub top_only: bool,
}

const HEADINGS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];
const CELLS: &[&str] = &["td", "th"];
const SECTIONS: &[&str] = &["thead", "tbody", "tfoot"];

pub(super) fn implied(tag: &str) -> Option<Implied> {
    let (closes, stops, top_only): (&[&str], &[&str], bool) = match tag {
        // A paragraph ends where a block starts, unless a cell, a button or the like sits between them.
        "p" | "address" | "article" | "aside" | "blockquote" | "details" | "div" | "dl"
        | "fieldset" | "figcaption" | "figure" | "footer" | "form" | "header" | "hr" | "main"
        | "menu" | "nav" | "ol" | "pre" | "section" | "table" | "ul" => (
            &["p"],
            &[
                "table", "td", "th", "caption", "button", "object", "template", "html", "body",
            ],
            false,
        ),
        "li" => (
            &["li"],
            &["ul", "ol", "menu", "table", "td", "th", "button"],
            false,
        ),
        "dd" | "dt" => (&["dd", "dt"], &["dl", "table", "td", "th", "button"], false),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => (HEADINGS, &[], true),
        "tr" => (&["tr"], &["table", "thead", "tbody", "tfoot"], false),
        "td" | "th" => (CELLS, &["tr", "table"], false),
        "thead" | "tbody" | "tfoot" => (SECTIONS, &["table"], false),
        "option" => (&["option"], &["select", "datalist"], false),
        "optgroup" => (&["option", "optgroup"], &["select"], false),
        _ => return None,
    };
    Some(Implied {
        closes,
        stops,
        top_only,
    })
}
