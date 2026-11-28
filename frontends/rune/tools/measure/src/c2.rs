//! C2 Text, the measurer part (C2a, section 5 of the spike note): the Rune measurer plugged into the conformance suite of E8.1.7
//! (`meldframe_layout::conformance::run_fixture`) over the text fixtures of the oracle, with the table recorded in Chromium by
//! the Linux oracle as the reference. It reports the width difference, the line count difference and the height of one line for
//! each fixture and writes every probe to a TSV file next to the JSON.
//!
//! C2b (the ink edges of painted text) and C2c (rounding of painted text to device pixels) need a painter that draws text, which
//! the skeleton does not: they are reported as not run. The verdict of C2 stays `open` until they are.

use crate::c2pop::{Populations, UNJUDGED, tsv};
use crate::files::{default_table_path, read_table, table_measurer, text_fixtures};
use crate::report::{Report, skipped};
use meldframe_layout::TextMeasurer;
use meldframe_layout::conformance::{FixtureReport, Tolerance, run_fixture};
use meldframe_text_rune::RuneMeasurer;
use std::path::{Path, PathBuf};

/// The limits the note gives C2a [E]: the same line breaks, and the width within 0.05 px on a single line. The height of one
/// line is reported and not judged [P], so its limit is none.
pub const TOLERANCE: Tolerance = Tolerance {
    width: 0.05,
    lines: 0,
    line_height: f64::INFINITY,
};

/// The widths of one text at the nine weights of the variable face, to see that the weight reaches the width.
const WEIGHT_TEXT: &str = "Weight reaches the width";

/// Runs C2 with the bundled fonts, `table` being the recorded reference (the default file when none is named) and `out` the
/// directory of the files.
pub fn run(table: Option<&Path>, out: &Path) -> Report {
    run_with(
        table,
        out,
        &RuneMeasurer::bundled_fonts_only(),
        "rune_bundled_inter",
    )
}

/// Runs C2 with `candidate`, named `name` in the labels.
pub fn run_with(table: Option<&Path>, out: &Path, candidate: &RuneMeasurer, name: &str) -> Report {
    let path: PathBuf = table.map_or_else(default_table_path, Path::to_path_buf);
    let table = match read_table(&path) {
        Ok(t) => t,
        Err(e) => {
            return skipped(
                "C2",
                &format!(
                    "the recorded Chromium table cannot be read ({e}); record it on Linux with tools/fidelity-oracle/record-text-table.mjs"
                ),
                &[],
            );
        }
    };
    let reference = table_measurer(&table);
    let reports: Vec<FixtureReport> = text_fixtures()
        .iter()
        .map(|f| run_fixture(&f.name, &f.nodes, candidate, &reference))
        .collect();
    let pops = Populations::of(&reports, TOLERANCE);

    let mut r = Report::new("C2");
    r.label("measurer", name).label(
        "reference",
        "pixi_CanvasTextMetrics_in_chromium_ceil_width_and_height",
    );
    r.word("c2a.reference_table", &table.recorded_with, &[]);
    let missed = reference.misses().len() as i64;
    r.int("c2a.reference_misses", missed, "count", &[]);
    for f in &reports {
        let mut one = Populations::default();
        for p in f.results.iter().filter(|p| p.in_contract) {
            one.all.add(p, TOLERANCE);
        }
        let judged = if f.name == UNJUDGED { "no" } else { "yes" };
        one.all.emit(
            &mut r,
            "fixture",
            &[("fixture", f.name.as_str()), ("judged", judged)],
        );
    }
    pops.spec.emit(
        &mut r,
        "spec_width_samples",
        &[
            ("population", "single_line_probes_of_f09_and_f15"),
            ("judged", "yes"),
        ],
    );
    pops.judged.emit(
        &mut r,
        "judged_fixtures",
        &[
            ("population", "in_contract_probes_of_every_fixture_but_f14"),
            ("judged", "line_breaks"),
        ],
    );
    pops.f14.emit(
        &mut r,
        "f14",
        &[
            ("population", "every_probe_of_f14_text_edge"),
            ("judged", "no"),
        ],
    );
    pops.all.emit(
        &mut r,
        "all",
        &[
            ("population", "in_contract_probes_of_the_nine_fixtures"),
            ("judged", "no"),
        ],
    );
    r.int(
        "c2a.probes_total",
        reports.iter().map(|f| f.results.len() as i64).sum(),
        "count",
        &[],
    )
    .word(
        "c2a.ink_case_samples",
        "not_available",
        &[(
            "reason",
            "the_17_ink_cases_and_their_hug_texts_are_not_exported_as_data",
        )],
    );
    weights(&mut r, candidate, &reference);
    let dump = out.join("c2-probes.tsv");
    let wrote = std::fs::create_dir_all(out).and_then(|()| {
        std::fs::write(
            &dump,
            tsv(&reports, &|p| {
                candidate.exact(&p.probe.node, p.probe.wrap_w).0
            }),
        )
    });
    r.word(
        "c2a.probe_table",
        &dump.display().to_string(),
        &[("written", if wrote.is_ok() { "yes" } else { "no" })],
    );
    if missed > 0 {
        r.status("partial", "the recorded table lacks answers the suite asked, so those probes compare with the stub")
            .verdict("open");
        return r;
    }
    r.word("c2a.verdict", pops.verdict(), &[("judged_on", "width_of_f09_f15_single_lines_and_line_breaks_of_all_but_f14")])
        .status("partial", "C2a ran against the recorded Chromium table; C2b and C2c need a painter that draws text, which the skeleton does not")
        .word("c2b.status", "skipped", &[("reason", "the_painter_draws_no_text")])
        .word("c2c.status", "skipped", &[("reason", "the_painter_draws_no_text")])
        .verdict("open");
    r
}

/// The widths of one text at the weights 100 to 900, and how many of them differ: a measurer that gives one width for every
/// weight does not let the weight reach the width (rule 5 of the contract), and the bundled face has three weights.
fn weights(r: &mut Report, candidate: &dyn TextMeasurer, reference: &dyn TextMeasurer) {
    let mut node = crate::files::text_fixtures()
        .into_iter()
        .flat_map(|f| f.nodes)
        .find(|n| !n.text.is_empty() && n.font_size > 0.0)
        .expect("a text node in the fixtures");
    node.text = WEIGHT_TEXT.into();
    node.font_size = 16.0;
    let (mut seen, mut seen_ref): (Vec<u64>, Vec<u64>) = (Vec::new(), Vec::new());
    for weight in (1..=9).map(|k| k * 100) {
        node.font_weight = weight;
        let w = candidate.measure(&node, None).0;
        let want = reference.measure(&node, None).0;
        r.num(
            "c2a.weight_width",
            w,
            "px",
            &[
                ("weight", &weight.to_string()),
                ("ref_width", &format!("{want}")),
            ],
        );
        if !seen.contains(&w.to_bits()) {
            seen.push(w.to_bits());
        }
        if !seen_ref.contains(&want.to_bits()) {
            seen_ref.push(want.to_bits());
        }
    }
    r.int(
        "c2a.weight_distinct_widths",
        seen.len() as i64,
        "count",
        &[("of", "9"), ("ref_distinct", &seen_ref.len().to_string())],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        std::env::temp_dir().join(format!("meldframe-measure-c2-{}", std::process::id()))
    }

    #[test]
    fn the_recorded_table_answers_every_question_the_suite_asks_of_every_fixture() {
        let dir = scratch();
        let text = run(None, &dir).text();
        std::fs::remove_dir_all(&dir).ok();
        assert!(
            text.contains("c2a.reference_misses 0 count"),
            "re-record the table: {text}"
        );
        assert_eq!(
            text.matches("c2a.fixture.probes ").count(),
            text_fixtures().len()
        );
    }

    #[test]
    fn c2_stays_open_while_the_painted_ink_is_not_measured_and_names_what_is_skipped() {
        let dir = scratch();
        let text = run(None, &dir).text();
        std::fs::remove_dir_all(&dir).ok();
        assert!(text.contains("C2 verdict: open"));
        assert!(text.contains("c2b.status skipped"));
        assert!(text.contains("c2c.status skipped"));
    }

    #[test]
    fn a_table_that_cannot_be_read_skips_the_criterion_and_prints_no_number() {
        let text = run(Some(Path::new("/nonexistent/table.json")), &scratch()).text();
        assert!(text.starts_with("C2 status: skipped"));
        assert!(!text.contains("c2a."));
    }

    #[test]
    fn the_stub_against_itself_is_the_exact_baseline_of_the_same_plumbing() {
        let reports: Vec<FixtureReport> = text_fixtures()
            .iter()
            .map(|f| {
                run_fixture(
                    &f.name,
                    &f.nodes,
                    &meldframe_layout::StubMeasurer,
                    &meldframe_layout::StubMeasurer,
                )
            })
            .collect();
        assert!(reports.iter().all(|r| r.failures(TOLERANCE).is_empty()));
    }

    /// Rune plugged into the suite against the committed table: the table answers every question (no probe compares with
    /// the stub), which says nothing about whether Rune is close to it.
    #[test]
    fn rune_in_the_suite_against_the_committed_table_has_no_reference_miss() {
        let table = read_table(&default_table_path()).expect("the committed table");
        let reference = table_measurer(&table);
        let rune = RuneMeasurer::bundled_fonts_only();
        for f in text_fixtures() {
            let report = run_fixture(&f.name, &f.nodes, &rune, &reference);
            assert!(!report.results.is_empty(), "{} asked nothing", f.name);
        }
        assert_eq!(reference.misses(), []);
    }

    /// A measurer uniformly twice as wide must fail the width limit on the note's samples and be judged a gap.
    #[test]
    fn a_measurer_twice_as_wide_as_rune_fails_the_width_samples_and_the_verdict_is_a_gap() {
        struct Twice(RuneMeasurer);
        impl TextMeasurer for Twice {
            fn measure(&self, node: &meldframe_protocol::Node, wrap_w: Option<f64>) -> (f64, f64) {
                let (w, h) = self.0.measure(node, wrap_w);
                (2.0 * w, h)
            }
        }
        let table = read_table(&default_table_path()).expect("the committed table");
        let reference = table_measurer(&table);
        let wide = Twice(RuneMeasurer::bundled_fonts_only());
        let reports: Vec<FixtureReport> = text_fixtures()
            .iter()
            .map(|f| run_fixture(&f.name, &f.nodes, &wide, &reference))
            .collect();
        let pops = Populations::of(&reports, TOLERANCE);
        assert!(pops.spec.singles > 0);
        assert_eq!(
            pops.spec.singles_over, pops.spec.singles,
            "every sample is over the limit"
        );
        assert_eq!(pops.verdict(), "gap");
    }

    /// The suite's own baseline through the populations: the stub against itself has nothing over any limit.
    #[test]
    fn the_stub_against_itself_has_no_difference_in_any_population() {
        let reports: Vec<FixtureReport> = text_fixtures()
            .iter()
            .map(|f| {
                run_fixture(
                    &f.name,
                    &f.nodes,
                    &meldframe_layout::StubMeasurer,
                    &meldframe_layout::StubMeasurer,
                )
            })
            .collect();
        let pops = Populations::of(&reports, TOLERANCE);
        assert_eq!(pops.verdict(), "green");
        assert_eq!(
            (
                pops.all.lines_differ,
                pops.all.lines_ambiguous,
                pops.all.height_differ
            ),
            (0, 0, 0)
        );
    }
}
