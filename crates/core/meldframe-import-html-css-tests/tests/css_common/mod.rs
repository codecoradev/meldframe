//! Helpers of the CSS tests: read text with the default limits, and write selectors, rules and declarations as
//! strings so that their shape is one string to compare.
#![allow(dead_code)]

use html_import::css::{
    Combinator, Decl, Limits, Pseudo, Report, Selector, Simple, Stylesheet, read, read_declarations,
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
