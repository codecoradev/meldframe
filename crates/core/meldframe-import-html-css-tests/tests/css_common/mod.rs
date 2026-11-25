//! Helpers of the CSS tests: read text with the default limits, and write selectors, rules and declarations as
//! strings so that their shape is one string to compare.
#![allow(dead_code)]

use html_import::css::{
    Candidate, Cascade, Combinator, Decl, Limits, Origin, Pseudo, Report, Selector, Simple, Source,
    Style, Styles, Stylesheet, cascade, read, read_declarations, style_tree,
};
use html_import::html::{ElementId, Parsed, parse};

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

/// A declaration as `name:value`, with `!` after an important one.
pub fn fmt_decl(d: &Decl) -> String {
    format!(
        "{}:{}{}",
        d.name,
        d.value,
        if d.important { "!" } else { "" }
    )
}

/// One rule a line: `sel, sel{name:value; name:value!}`.
pub fn dump(s: &Stylesheet) -> Vec<String> {
    s.rules
        .iter()
        .map(|r| {
            let sels: Vec<String> = r.selectors.iter().map(sel).collect();
            let decls: Vec<String> = r.decls.iter().map(fmt_decl).collect();
            format!("{}{{{}}}", sels.join(", "), decls.join("; "))
        })
        .collect()
}

pub fn lines(css: &str) -> Vec<String> {
    dump(&sheet(css).0)
}

/// The declarations of a `style` attribute value, and what the reader said.
pub fn decls(text: &str) -> (Vec<String>, Report) {
    decls_with(text, &Limits::default())
}

pub fn decls_with(text: &str, limits: &Limits) -> (Vec<String>, Report) {
    let mut report = Report::default();
    let d = read_declarations(text, 0, limits, &mut report);
    (d.iter().map(fmt_decl).collect(), report)
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
            let mark = if p.source == Source::Inherited {
                "^"
            } else {
                ""
            };
            format!("{k}:{}{mark}", p.value)
        })
        .collect()
}
