//! Helpers of the paint tests: read an element's paint from HTML and CSS, and look at the report.
#![allow(dead_code)]

use html_import::css::{EntryKind, Limits, Report, Style, style_tree};
use html_import::html::parse;
use html_import::paint::{self, Ctx, Paint};
use meldframe_protocol::{Bind, Gradient, GradientKind, GradientStop};

pub struct Read {
    pub paint: Paint,
    pub report: Report,
    pub style: Style,
}

/// The paint of the element `id=t` of `html`, with `css` as the extra sheet and `ctx` for the box.
pub fn read_in(html: &str, css: &str, ctx: &Ctx) -> Read {
    let parsed = parse(html, &Default::default()).expect("html");
    let mut report = Report::default();
    let styles = style_tree(&parsed.tree, css, &Limits::default(), &mut report);
    let id = parsed
        .tree
        .ids()
        .find(|i| parsed.tree.element(*i).attr("id") == Some("t"))
        .expect("an element with id=t");
    let at = parsed.tree.element(id).at;
    let style = styles.get(id).clone();
    let before = report.entries.len();
    let paint = paint::read(&style, ctx, at, &mut report);
    report.entries.drain(..before);
    Read {
        paint,
        report,
        style,
    }
}

/// The paint of `<div id=t style="{decls}">x</div>`.
pub fn read(decls: &str) -> Read {
    read_ctx(decls, &Ctx::default())
}

pub fn read_ctx(decls: &str, ctx: &Ctx) -> Read {
    read_in(&format!("<div id=t style=\"{decls}\">x</div>"), "", ctx)
}

/// The paint with a rule for `#t` and for its `::after` in the extra sheet.
pub fn read_css(css: &str) -> Read {
    read_in("<div id=t>x</div>", css, &Ctx::default())
}

impl Read {
    /// `kind:code` of every entry the paint stage added, in order.
    pub fn notes(&self) -> Vec<String> {
        self.report
            .entries
            .iter()
            .map(|e| format!("{}:{}", kind(e.kind), e.code))
            .collect()
    }

    pub fn has(&self, kind_code: &str) -> bool {
        self.notes().iter().any(|n| n == kind_code)
    }

    pub fn fill(&self) -> &str {
        match &self.paint.fill {
            Bind::Lit(s) => s,
            Bind::Var(_) => panic!("a bound fill"),
        }
    }

    pub fn gradient(&self) -> &Gradient {
        self.paint.gradient.as_ref().expect("a gradient")
    }
}

fn kind(k: EntryKind) -> &'static str {
    k.as_str()
}

pub fn stops(list: &[(f64, &str)]) -> Vec<GradientStop> {
    list.iter()
        .map(|(pos, color)| GradientStop {
            pos: *pos,
            color: (*color).into(),
        })
        .collect()
}

pub fn linear(angle: f64, list: &[(f64, &str)]) -> Gradient {
    Gradient {
        kind: GradientKind::Linear,
        angle,
        stops: stops(list),
    }
}

pub fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}
