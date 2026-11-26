//! Helpers of the HTML import tests: parse with the default limits, and write a tree as one line so that its shape is
//! one string to compare.
#![allow(dead_code)]

use html_import::html::{Child, ElementId, Limits, ParseError, Parsed, Tree, parse};

/// The most frames the builder looks at for one tag (the default depth limit): the work of any input is within
/// this many frames for each byte of it.
pub const WINDOW: usize = 64;

pub fn run(html: &str) -> Parsed {
    parse(html, &Limits::default()).unwrap_or_else(|e| panic!("{e} for {html:?}"))
}

pub fn refused(html: &str, limits: &Limits) -> ParseError {
    match parse(html, limits) {
        Ok(p) => panic!("parsed to {}", dump(&p.tree)),
        Err(e) => e,
    }
}

/// `div(class=a){p{"text"} br}`: a tag, its attributes in order, then its children in braces. A text is quoted by
/// [`q`]. The root has no tag of its own, so its children are written bare, separated by a space.
pub fn dump(tree: &Tree) -> String {
    let mut out = String::new();
    children(tree, tree.root_id(), &mut out);
    out
}

fn children(tree: &Tree, id: ElementId, out: &mut String) {
    for (k, c) in tree.element(id).children.iter().enumerate() {
        if k > 0 {
            out.push(' ');
        }
        match c {
            Child::Text(t) => out.push_str(&q(t)),
            Child::Element(e) => {
                let el = tree.element(*e);
                out.push_str(&el.tag);
                if !el.attrs.is_empty() {
                    let a: Vec<String> = el
                        .attrs
                        .iter()
                        .map(|a| format!("{}={}", a.name, a.value))
                        .collect();
                    out.push_str(&format!("({})", a.join(" ")));
                }
                if !el.children.is_empty() {
                    out.push('{');
                    children(tree, *e, out);
                    out.push('}');
                }
            }
        }
    }
}

pub fn shape(html: &str) -> String {
    dump(&run(html).tree)
}

/// How often the report counted `code`.
pub fn count(p: &Parsed, code: &str) -> usize {
    p.report.counts.get(code).copied().unwrap_or(0)
}

/// Limits for a test of one bound: everything else is raised out of the way.
pub fn roomy() -> Limits {
    Limits {
        max_input_bytes: usize::MAX,
        max_elements: usize::MAX,
        max_depth: 64,
        max_attributes: 64,
        max_value_bytes: 1_000,
        max_class_bytes: 8 * 1024,
        max_text_bytes: usize::MAX,
    }
}

/// The first element of the root.
pub fn first_element(tree: &Tree) -> ElementId {
    tree.root()
        .children
        .iter()
        .find_map(|c| match c {
            Child::Element(e) => Some(*e),
            Child::Text(_) => None,
        })
        .expect("an element")
}

/// A text in quotes, with only `"`, `\`, line feed, tab and carriage return escaped (what `{:?}` does with a
/// character that is not printable would make the expected strings hard to read).
pub fn q(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
