//! Helpers of the CSS reader tests: read text with the default limits, and write a sheet as lines so that its
//! shape is one string to compare.
#![allow(dead_code)]

use html_import::css::{
    Combinator, Limits, Pseudo, Report, Selector, Simple, Stylesheet, read, read_declarations,
};

pub fn sheet(css: &str) -> (Stylesheet, Report) {
    sheet_with(css, &Limits::default())
}

pub fn sheet_with(css: &str, limits: &Limits) -> (Stylesheet, Report) {
    let mut report = Report::default();
    let s = read([css], limits, &mut report);
    (s, report)
}

/// A selector as text: `div>.a #b[x=y]::after`. Specificity is written by [`spec`].
pub fn sel(s: &Selector) -> String {
    let mut out = String::new();
    for (i, c) in s.compounds.iter().enumerate() {
        if i > 0 {
            out.push(match s.combinators[i - 1] {
                Combinator::Descendant => ' ',
                Combinator::Child => '>',
            });
        }
        if c.simples.is_empty() {
            out.push('*');
        }
        for simple in &c.simples {
            match simple {
                Simple::Tag(t) => out.push_str(t),
                Simple::Class(c) => out.push_str(&format!(".{c}")),
                Simple::Id(i) => out.push_str(&format!("#{i}")),
                Simple::Attr(n, None) => out.push_str(&format!("[{n}]")),
                Simple::Attr(n, Some(v)) => out.push_str(&format!("[{n}={v}]")),
                Simple::Root => out.push_str(":root"),
            }
        }
    }
    match s.pseudo {
        Some(Pseudo::Before) => out.push_str("::before"),
        Some(Pseudo::After) => out.push_str("::after"),
        None => {}
    }
    out
}

/// The specificity as `(ids, classes, types)`.
pub fn spec(s: &Selector) -> (u32, u32, u32) {
    (
        s.specificity >> 16,
        (s.specificity >> 8) & 255,
        s.specificity & 255,
    )
}

/// One rule a line: `sel, sel{name:value; name:value!}` (`!` marks important).
pub fn dump(s: &Stylesheet) -> Vec<String> {
    s.rules
        .iter()
        .map(|r| {
            let sels: Vec<String> = r.selectors.iter().map(sel).collect();
            let decls: Vec<String> = r
                .decls
                .iter()
                .map(|d| {
                    format!(
                        "{}:{}{}",
                        d.name,
                        d.value,
                        if d.important { "!" } else { "" }
                    )
                })
                .collect();
            format!("{}{{{}}}", sels.join(", "), decls.join("; "))
        })
        .collect()
}

pub fn lines(css: &str) -> Vec<String> {
    dump(&sheet(css).0)
}

pub fn decls(text: &str) -> (Vec<String>, Report) {
    let mut report = Report::default();
    let d = read_declarations(text, &Limits::default(), &mut report);
    let out = d
        .iter()
        .map(|d| {
            format!(
                "{}:{}{}",
                d.name,
                d.value,
                if d.important { "!" } else { "" }
            )
        })
        .collect();
    (out, report)
}

/// How often the report counted `code`.
pub fn count(r: &Report, code: &str) -> usize {
    r.counts.get(code).copied().unwrap_or(0)
}

/// The names the report lists under `code`, in order.
pub fn names<'a>(r: &'a Report, code: &str) -> Vec<&'a str> {
    r.entries
        .iter()
        .filter(|e| e.code == code)
        .map(|e| e.name.as_str())
        .collect()
}

use html_import::css::{Candidate, Cascade, Origin, cascade};
use html_import::html::{Child, ElementId, Parsed, parse};

/// Read `html` and `css` and cascade them with `limits`.
pub fn cascaded_with(html: &str, css: &str, limits: &Limits) -> (Parsed, Cascade, Report) {
    let parsed = parse(html, &Default::default()).expect("html");
    let mut report = Report::default();
    let sheet = read(parsed.tree.style_texts().chain([css]), limits, &mut report);
    let c = cascade(&parsed.tree, &sheet, limits, &mut report);
    (parsed, c, report)
}

pub fn cascaded(html: &str, css: &str) -> (Parsed, Cascade, Report) {
    cascaded_with(html, css, &Limits::default())
}

/// The first element with this tag, in the order of the text.
pub fn el(p: &Parsed, tag: &str) -> ElementId {
    p.tree
        .ids()
        .find(|i| p.tree.element(*i).tag == tag)
        .unwrap_or_else(|| panic!("no <{tag}>"))
}

/// The candidates as `name:value`, lowest first, with `!` for important and `@` for inline.
pub fn list(c: &[Candidate]) -> Vec<String> {
    c.iter()
        .map(|c| {
            format!(
                "{}:{}{}{}",
                c.name,
                c.value,
                if c.important { "!" } else { "" },
                if c.origin == Origin::Inline { "@" } else { "" }
            )
        })
        .collect()
}

/// What applies to the first `<tag>`.
pub fn main(html: &str, css: &str, tag: &str) -> Vec<String> {
    let (p, c, _) = cascaded(html, css);
    list(&c.elements[el(&p, tag).index()].main)
}

pub fn text_children(p: &Parsed, id: ElementId) -> usize {
    p.tree
        .element(id)
        .children
        .iter()
        .filter(|c| matches!(c, Child::Text(_)))
        .count()
}

use html_import::css::{Style, Styles, style_tree};

/// Read `html` and `css`, cascade and compute with `limits`.
pub fn styled_with(html: &str, css: &str, limits: &Limits) -> (Parsed, Styles, Report) {
    let parsed = parse(html, &Default::default()).expect("html");
    let mut report = Report::default();
    let s = style_tree(&parsed.tree, css, limits, &mut report);
    (parsed, s, report)
}

pub fn styled(html: &str, css: &str) -> (Parsed, Styles, Report) {
    styled_with(html, css, &Limits::default())
}

/// The style of the first `<tag>`.
pub fn style_of(html: &str, css: &str, tag: &str) -> Style {
    let (p, s, _) = styled(html, css);
    s.get(el(&p, tag)).clone()
}

/// The properties of a style as `name:value`, declared ones with `^` for inherited, sorted by name.
pub fn props(s: &Style) -> Vec<String> {
    s.props
        .iter()
        .map(|(k, p)| {
            let mark = if p.source == html_import::css::Source::Inherited {
                "^"
            } else {
                ""
            };
            format!("{k}:{}{mark}", p.value)
        })
        .collect()
}
