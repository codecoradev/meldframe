//! Helpers of the style tests: read `html` and `css`, cascade and compute, and look at the result.
#![allow(dead_code)]

use html_import::css::{Limits, Report, Source, Style, Styles, cascade, read, style_tree};
use html_import::html::{ElementId, Parsed, parse};

/// How often the report counted `code`.
pub fn count(r: &Report, code: &str) -> usize {
    r.counts.get(code).copied().unwrap_or(0)
}

/// The first element with this tag, in the order of the text.
pub fn el(p: &Parsed, tag: &str) -> ElementId {
    p.tree
        .ids()
        .find(|i| p.tree.element(*i).tag == tag)
        .unwrap_or_else(|| panic!("no <{tag}>"))
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

/// The steps and bytes the style stage took apart from reading and the cascade: it is run on its own.
pub fn style_cost(html: &str, css: &str, limits: &Limits) -> (Styles, usize, Report) {
    let parsed = parse(html, &Default::default()).expect("html");
    let mut report = Report::default();
    let sheet = read(parsed.tree.style_texts().chain([css]), limits, &mut report);
    let cascaded = cascade(&parsed.tree, &sheet, limits, &mut report);
    let mut own = Report::default();
    let styles = html_import::css::compute(&parsed.tree, &cascaded, limits, &mut own);
    let work = own.work;
    (styles, work, own)
}
