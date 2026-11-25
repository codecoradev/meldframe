//! Helpers of the CSS tests: read text with the default limits and write the result as strings, so that its shape
//! is one string to compare.
#![allow(dead_code)]

use html_import::css::{Decl, Limits, Report, read_declarations};

/// A declaration as `name:value`, with `!` after an important one.
pub fn fmt_decl(d: &Decl) -> String {
    format!(
        "{}:{}{}",
        d.name,
        d.value,
        if d.important { "!" } else { "" }
    )
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
