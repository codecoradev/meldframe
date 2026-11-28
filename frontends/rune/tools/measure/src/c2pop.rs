//! The populations of C2a and what is counted in each (section 5 of the spike note). The note judges the width on the
//! single-line hug texts of the ink cases, `f09-typography` and `f15-font-sizes`, and the line breaks on every fixture the contract
//! covers; `f14-text-edge` is reported and never judged. The ink cases are not exported as data, so the width population here is
//! the `f09` and `f15` samples alone, and a wider one (every fixture but `f14`) is printed beside it, with `f14` and the whole set
//! apart, so that nothing is hidden by the choice.
//!
//! The line count of a probe is derived by the suite as `round(height / height of one M)`. That is exact for a measurer whose
//! lines are all the pinned height (the Rune measurer), and only approximate for the reference, whose heights are `ceil(lines *
//! Pixi's fractional line height)` and whose `M` is rounded up. A difference of one line is therefore `confirmed` only when the
//! reference's own ratio is within a tenth of a whole number; otherwise it is `ambiguous` (it may be the height and not the
//! breaks) and is printed and not judged. The seam does not expose the break count, which is the limit of this check.

use crate::report::Report;
use meldframe_layout::conformance::{FixtureReport, ProbeResult, Tolerance};

/// The fixture the note reports and never judges.
pub const UNJUDGED: &str = "f14-text-edge";

/// The fixtures whose single-line hug texts are the note's width samples that exist as data.
pub const WIDTH_SAMPLES: [&str; 2] = ["f09-typography", "f15-font-sizes"];

/// How a line count difference reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lines {
    Same,
    /// The counts differ and the reference's derived count is not near a half: the breaks differ.
    Differs,
    /// The counts differ but the reference's ratio is near a half, so the height may be the cause.
    Ambiguous,
}

/// Whether `r`'s line counts agree, and if not whether the derivation can be trusted.
pub fn line_check(r: &ProbeResult) -> Lines {
    if r.line_diff() == 0 {
        return Lines::Same;
    }
    let ratio = |h: f64, line: f64| if line > 0.0 { h / line } else { f64::NAN };
    let off = |x: f64| (x - x.round()).abs();
    let (reference, candidate) = (
        ratio(r.reference.1, r.reference_line_height),
        ratio(r.candidate.1, r.candidate_line_height),
    );
    if off(reference) <= 0.1 && off(candidate) <= 0.1 {
        Lines::Differs
    } else {
        Lines::Ambiguous
    }
}

/// What is counted over one set of probes.
#[derive(Debug, Default, Clone)]
pub struct Pop {
    pub probes: i64,
    pub singles: i64,
    pub singles_over: i64,
    pub single_width_max: f64,
    pub width_max: f64,
    pub lines_differ: i64,
    pub lines_ambiguous: i64,
    pub height_differ: i64,
    pub height_max: f64,
    pub line_height_differ: i64,
    pub line_height_max: f64,
}

impl Pop {
    /// Adds one probe.
    pub fn add(&mut self, r: &ProbeResult, tolerance: Tolerance) {
        self.probes += 1;
        let (w, h, lh) = (
            r.width_diff().abs(),
            (r.candidate.1 - r.reference.1).abs(),
            r.line_height_diff().abs(),
        );
        self.width_max = self.width_max.max(w);
        if r.probe.wrap_w.is_none() {
            self.singles += 1;
            self.singles_over += i64::from(w > tolerance.width);
            self.single_width_max = self.single_width_max.max(w);
        }
        match line_check(r) {
            Lines::Same => {}
            Lines::Differs => self.lines_differ += 1,
            Lines::Ambiguous => self.lines_ambiguous += 1,
        }
        self.height_differ += i64::from(h > tolerance.width);
        self.height_max = self.height_max.max(h);
        self.line_height_differ += i64::from(lh > tolerance.width);
        self.line_height_max = self.line_height_max.max(lh);
    }

    /// Prints the counts under `name` with the labels.
    pub fn emit(&self, r: &mut Report, name: &str, labels: &[(&str, &str)]) {
        let n = |k: &str| format!("c2a.{name}.{k}");
        r.int(&n("probes"), self.probes, "count", labels)
            .int(&n("single_line_samples"), self.singles, "count", labels)
            .int(
                &n("single_line_width_over_0.05px"),
                self.singles_over,
                "count",
                labels,
            )
            .num(
                &n("single_line_width_diff_max"),
                self.single_width_max,
                "px",
                labels,
            )
            .num(&n("width_diff_max"), self.width_max, "px", labels)
            .int(&n("line_count_differs"), self.lines_differ, "count", labels)
            .int(
                &n("line_count_ambiguous"),
                self.lines_ambiguous,
                "count",
                labels,
            )
            .int(
                &n("height_over_0.05px"),
                self.height_differ,
                "count",
                labels,
            )
            .num(&n("height_diff_max"), self.height_max, "px", labels)
            .int(
                &n("line_height_over_0.05px"),
                self.line_height_differ,
                "count",
                labels,
            )
            .num(
                &n("line_height_diff_max"),
                self.line_height_max,
                "px",
                labels,
            );
    }
}

/// The populations of one run.
#[derive(Debug, Default)]
pub struct Populations {
    /// The note's width samples that exist as data: single-line probes of `f09` and `f15`.
    pub spec: Pop,
    /// Every probe inside the contract of every fixture but `f14`: the set the line breaks are judged on.
    pub judged: Pop,
    /// Every probe of `f14`, inside the contract or not: reported only.
    pub f14: Pop,
    /// Every probe inside the contract of the nine fixtures.
    pub all: Pop,
}

impl Populations {
    /// Sorts the probes of the reports into the sets.
    pub fn of(reports: &[FixtureReport], tolerance: Tolerance) -> Populations {
        let mut p = Populations::default();
        for f in reports {
            for r in &f.results {
                if f.name == UNJUDGED {
                    p.f14.add(r, tolerance);
                }
                if !r.in_contract {
                    continue;
                }
                p.all.add(r, tolerance);
                if f.name != UNJUDGED {
                    p.judged.add(r, tolerance);
                }
                if WIDTH_SAMPLES.contains(&f.name.as_str()) && r.probe.wrap_w.is_none() {
                    p.spec.add(r, tolerance);
                }
            }
        }
        p
    }

    /// The verdict of C2a on what the note judges: the width on the samples, the line breaks on the judged set. The height of a
    /// line is reported and not judged [P] and is not part of it.
    pub fn verdict(&self) -> &'static str {
        if self.spec.singles_over == 0 && self.judged.lines_differ == 0 {
            "green"
        } else {
            "gap"
        }
    }
}

/// One TSV row per probe, with the unrounded width of the candidate and how its line count reads.
pub fn tsv(reports: &[FixtureReport], exact: &dyn Fn(&ProbeResult) -> f64) -> String {
    let mut out = String::from(
        "fixture\tnode\ttext\tfont_size\tweight\twrap\tin_contract\tref_w\tref_h\tcand_w\tcand_h\tcand_w_exact\tref_lines\tcand_lines\tlines\tref_line_h\tcand_line_h\n",
    );
    for f in reports {
        for r in &f.results {
            let n = &r.probe.node;
            let text: String = n
                .text
                .chars()
                .take(24)
                .map(|c| {
                    if matches!(c, '\n' | '\t' | '\r') {
                        ' '
                    } else {
                        c
                    }
                })
                .collect();
            let wrap = r.probe.wrap_w.map_or("none".into(), |w| format!("{w:.4}"));
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.4}\t{}\t{}\t{:?}\t{}\t{}\n",
                f.name,
                n.id,
                text,
                n.font_size,
                n.font_weight,
                wrap,
                r.in_contract,
                r.reference.0,
                r.reference.1,
                r.candidate.0,
                r.candidate.1,
                exact(r),
                r.reference_lines,
                r.candidate_lines,
                line_check(r),
                r.reference_line_height,
                r.candidate_line_height,
            ));
        }
    }
    out
}
