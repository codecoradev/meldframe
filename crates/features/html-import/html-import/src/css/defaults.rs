//! What no rule says: the browser's own defaults, as a small fixed table (`docs/import-html.md` section 3.3), the
//! properties that inherit, and the initial value of those that do. Anything not here has CSS's initial value, not
//! the browser's: a `ul` has its margin and padding, a `button` has no chrome.
//!
//! The model's own defaults are not CSS's (`fill` `#3f3f46`, `clip` true, `padding` 16, `gap` 8; section 3.1). They
//! are not applied here: a property with no value stays absent in the computed style, and the mapping writes the
//! model's field explicitly (`fill` none, `clip` false, `padding` 0, `gap` 0 unless authored).

use crate::html::Element;

/// The properties that pass to children: the five of the note, `font-family` and `line-height` (read, then refused
/// where they land), and the other inherited ones the mapping refuses by name.
pub const INHERITED: [&str; 14] = [
    "color",
    "cursor",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
    "letter-spacing",
    "line-height",
    "text-align",
    "text-indent",
    "text-transform",
    "visibility",
    "white-space",
    "word-break",
];

pub fn inherits(prop: &str) -> bool {
    INHERITED.contains(&prop)
}

/// The initial value of an inherited property, for `initial`. `None`: it has none the mapping needs (the property
/// is left out).
pub fn initial(prop: &str) -> Option<&'static str> {
    Some(match prop {
        "color" => "#000000",
        "font-size" => "16px",
        "font-weight" => "400",
        "text-align" => "start",
        "white-space" => "normal",
        "line-height" => "normal",
        "letter-spacing" => "normal",
        "text-indent" => "0",
        "text-transform" => "none",
        "visibility" => "visible",
        "font-style" => "normal",
        "word-break" => "normal",
        "cursor" => "auto",
        _ => return None,
    })
}

type Decls = &'static [(&'static str, &'static str)];

const BLOCK: Decls = &[("display", "block")];
const HIDDEN: Decls = &[("display", "none")];

/// The defaults of an element: declarations, lowest priority of all (under author rules and the `style`
/// attribute). `h1` to `h6`, `strong` and `b`, `p`, `ul`, `ol`, `dl`, `blockquote`, `pre`, `figure`, `th`, `a`,
/// `small` and `body`, and the `display` of the rest.
pub fn ua(el: &Element) -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = match el.tag.as_str() {
        "head" | "script" | "style" | "link" | "meta" | "title" | "noscript" | "template"
        | "base" | "area" | "param" | "source" | "track" => HIDDEN.to_vec(),
        "body" => vec![("display", "block"), ("margin", "8px")],
        "h1" => heading("2em", "0.67em 0"),
        "h2" => heading("1.5em", "0.83em 0"),
        "h3" => heading("1.17em", "1em 0"),
        "h4" => heading("1em", "1.33em 0"),
        "h5" => heading("0.83em", "1.67em 0"),
        "h6" => heading("0.67em", "2.33em 0"),
        "p" | "dl" => vec![("display", "block"), ("margin", "1em 0")],
        "ul" | "ol" => vec![
            ("display", "block"),
            ("margin", "1em 0"),
            ("padding-left", "40px"),
        ],
        "blockquote" | "figure" => vec![("display", "block"), ("margin", "1em 40px")],
        "dd" => vec![("display", "block"), ("margin-left", "40px")],
        "pre" => vec![
            ("display", "block"),
            ("margin", "1em 0"),
            ("white-space", "pre"),
        ],
        "li" => vec![("display", "list-item")],
        "strong" | "b" => vec![("font-weight", "bold")],
        "th" => vec![
            ("display", "table-cell"),
            ("font-weight", "bold"),
            ("text-align", "center"),
        ],
        "small" => vec![("font-size", "0.83em")],
        "a" if el.attr("href").is_some() => vec![("color", "#0000ee")],
        "table" => vec![("display", "table")],
        "caption" => vec![("display", "table-caption")],
        "thead" => vec![("display", "table-header-group")],
        "tbody" => vec![("display", "table-row-group")],
        "tfoot" => vec![("display", "table-footer-group")],
        "tr" => vec![("display", "table-row")],
        "td" => vec![("display", "table-cell")],
        "colgroup" => vec![("display", "table-column-group")],
        "col" => vec![("display", "table-column")],
        "html" | "div" | "section" | "article" | "header" | "footer" | "main" | "nav" | "aside"
        | "form" | "fieldset" | "figcaption" | "address" | "dt" | "details" | "summary"
        | "menu" | "hgroup" | "hr" => BLOCK.to_vec(),
        _ => Vec::new(),
    };
    if el.attr("hidden").is_some() || (el.tag == "input" && el.attr("type") == Some("hidden")) {
        out.push(("display", "none"));
    }
    out
}

fn heading(size: &'static str, margin: &'static str) -> Vec<(&'static str, &'static str)> {
    vec![
        ("display", "block"),
        ("font-size", size),
        ("font-weight", "bold"),
        ("margin", margin),
    ]
}
