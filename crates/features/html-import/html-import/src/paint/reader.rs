//! The reading of one element's paint from its computed style, and the sink that says what it left out.

use super::{Ctx, Paint, border, fill, misc, radius, shadow};
use crate::css::{EntryKind, Report, Style};

/// Where the reading says what it left out: the report, the byte of the element, and the work done.
pub(crate) struct Sink<'a> {
    pub report: &'a mut Report,
    pub at: usize,
    pub steps: usize,
}

impl Sink<'_> {
    fn note(&mut self, kind: EntryKind, code: &'static str, prop: &str, value: &str, why: &str) {
        let value: String = value.chars().take(80).collect();
        let msg = format!("{prop}: {value}: {why}");
        self.report.note(kind, code, prop, msg, self.at);
    }

    /// A property that is lost.
    pub fn refuse(&mut self, code: &'static str, prop: &str, value: &str, why: &str) {
        self.note(EntryKind::Refused, code, prop, value, why);
    }

    /// A value that was changed to the nearest the model holds (not a lost property).
    pub fn list(&mut self, code: &'static str, prop: &str, value: &str, why: &str) {
        self.note(EntryKind::Listed, code, prop, value, why);
    }

    /// A bound stopped the reading.
    pub fn cut(&mut self, code: &'static str, prop: &str, value: &str, why: &str) {
        self.note(EntryKind::Cut, code, prop, value, why);
    }
}

/// Read the paint of one element from its computed style.
///
/// - `ctx`: what the box is (see [`Ctx`]);
/// - `at`: the byte of the document where the element starts, for the report;
/// - `report`: receives what is refused, listed or cut, and the work in `report.work`.
///
/// Never fails and never panics; the work is bounded by the size of the values, which the CSS reader already bounds.
pub fn read(style: &Style, ctx: &Ctx, at: usize, report: &mut Report) -> Paint {
    let mut paint = Paint::default();
    let mut k = Sink {
        report,
        at,
        steps: 0,
    };
    fill::read(style, ctx, &mut paint, &mut k);
    let from_box = border::read(style, &mut paint, &mut k);
    radius::read(style, ctx, from_box, &mut paint, &mut k);
    shadow::read(style, &mut paint, &mut k);
    misc::opacity(style, &mut paint, &mut k);
    misc::refused(style, &mut k);
    k.report.work += k.steps;
    paint
}
