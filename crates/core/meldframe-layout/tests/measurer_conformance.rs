//! The conformance suite of a text measurer: the stub passes its own baseline over the texts of the oracle
//! fixtures, and measurers that are wrong in a known way fail it.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_layout::conformance::{FixtureReport, Tolerance, run_fixture};
use meldframe_layout::{MAX_MEASURE, StubMeasurer, TextMeasurer};
use meldframe_protocol::Node;
use serde::Deserialize;

const FIXTURES: &str = include_str!("fixtures/oracle_texts.json");

#[derive(Deserialize)]
struct Fixture {
    name: String,
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct File {
    fixtures: Vec<Fixture>,
}

fn fixtures() -> Vec<Fixture> {
    serde_json::from_str::<File>(FIXTURES)
        .expect("a valid fixture file")
        .fixtures
}

fn suite(candidate: &dyn TextMeasurer, reference: &dyn TextMeasurer) -> Vec<FixtureReport> {
    fixtures()
        .iter()
        .map(|f| run_fixture(&f.name, &f.nodes, candidate, reference))
        .collect()
}

fn failing(reports: &[FixtureReport], tolerance: Tolerance) -> Vec<&str> {
    reports
        .iter()
        .filter(|r| !r.failures(tolerance).is_empty())
        .map(|r| r.name.as_str())
        .collect()
}

/// Right line counts and heights, but every width is 20 % too large.
struct WiderGlyphs;

impl TextMeasurer for WiderGlyphs {
    fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
        let (w, h) = StubMeasurer.measure(node, wrap_w);
        ((w * 1.2).ceil(), h)
    }
}

/// The stub's rule at a font size 1.2 times larger and the same wrap width: wider text that wraps earlier.
struct LargerFont;

impl TextMeasurer for LargerFont {
    fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
        let mut big = node.clone();
        big.font_size = node.font_size * 1.2;
        StubMeasurer.measure(&big, wrap_w)
    }
}

/// Right widths and the right number of lines, but every line twice as tall.
struct TallLines;

impl TextMeasurer for TallLines {
    fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
        let (w, h) = StubMeasurer.measure(node, wrap_w);
        (w, h * 2.0)
    }
}

/// Right widths for one line, but never wraps: every text is one line high.
struct NeverWraps;

impl TextMeasurer for NeverWraps {
    fn measure(&self, node: &Node, _wrap_w: Option<f64>) -> (f64, f64) {
        StubMeasurer.measure(node, None)
    }
}

/// Answers a width that is not a number.
struct NotASize;

impl TextMeasurer for NotASize {
    fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
        (f64::NAN, StubMeasurer.measure(node, wrap_w).1)
    }
}

/// Answers a huge negative height: a finite number that is not a size.
struct HugeNegative;

impl TextMeasurer for HugeNegative {
    fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
        (StubMeasurer.measure(node, wrap_w).0, -1.0e300)
    }
}

/// Answers sizes that are a little over the largest a host may answer.
struct OverTheCap;

impl TextMeasurer for OverTheCap {
    fn measure(&self, _node: &Node, _wrap_w: Option<f64>) -> (f64, f64) {
        (MAX_MEASURE + 1.0, 20.0)
    }
}

#[test]
fn the_fixtures_hold_texts_that_wrap_and_texts_outside_the_contract() {
    let reports = suite(&StubMeasurer, &StubMeasurer);
    assert!(reports.len() >= 8, "the fixture file lost fixtures");
    let results: Vec<_> = reports.iter().flat_map(|r| &r.results).collect();
    assert!(results.len() >= 40, "only {} probes", results.len());
    assert!(
        results.iter().any(|r| r.probe.wrap_w.is_some()),
        "no wrapped text"
    );
    assert!(
        results.iter().any(|r| r.reference_lines > 1),
        "no text of several lines"
    );
    assert!(
        results.iter().any(|r| !r.in_contract),
        "no text outside the contract"
    );
}

#[test]
fn the_stub_has_no_difference_from_itself_on_any_fixture() {
    for report in suite(&StubMeasurer, &StubMeasurer) {
        assert!(!report.results.is_empty(), "{}: no probe", report.name);
        assert!(
            report.failures(Tolerance::EXACT).is_empty(),
            "{}",
            report.name
        );
        assert_eq!(report.max_width_diff(), 0.0, "{}", report.name);
        assert_eq!(report.max_line_diff(), 0, "{}", report.name);
        assert_eq!(report.max_line_height_diff(), 0.0, "{}", report.name);
    }
}

#[test]
fn wrong_widths_with_right_lines_fail_on_the_width_alone() {
    let reports = suite(&WiderGlyphs, &StubMeasurer);
    assert!(failing(&reports, Tolerance::EXACT).len() >= 5);
    assert!(reports.iter().any(|r| r.max_width_diff() > 1.0));
    for r in &reports {
        assert_eq!(r.max_line_diff(), 0, "{}", r.name);
        assert_eq!(r.max_line_height_diff(), 0.0, "{}", r.name);
    }
}

#[test]
fn a_larger_font_fails_on_the_width_and_on_the_lines_it_wraps_differently() {
    let reports = suite(&LargerFont, &StubMeasurer);
    assert!(reports.iter().any(|r| r.max_width_diff() > 1.0));
    assert!(reports.iter().any(|r| r.max_line_diff() >= 1));
    assert!(reports.iter().any(|r| r.max_line_height_diff() > 0.0));
}

#[test]
fn lines_that_are_all_too_tall_have_the_right_count_and_fail_on_the_line_height() {
    let reports = suite(&TallLines, &StubMeasurer);
    for r in &reports {
        assert_eq!(
            r.max_line_diff(),
            0,
            "{}: the count follows the heights",
            r.name
        );
        assert_eq!(r.max_width_diff(), 0.0, "{}", r.name);
    }
    assert_eq!(failing(&reports, Tolerance::EXACT).len(), reports.len());
    assert!(reports.iter().all(|r| r.max_line_height_diff() > 0.0));
    // The line height has a limit of its own: the other two are not enough to forgive it.
    let no_height = Tolerance {
        line_height: 0.0,
        ..loose()
    };
    assert_eq!(failing(&reports, no_height).len(), reports.len());
    assert!(failing(&reports, loose()).is_empty());
}

/// Every limit wide open.
fn loose() -> Tolerance {
    Tolerance {
        width: 1.0e9,
        lines: 1000,
        line_height: 1.0e9,
    }
}

#[test]
fn a_measurer_that_never_wraps_fails_on_the_line_count_and_not_on_a_loose_width_tolerance() {
    let reports = suite(&NeverWraps, &StubMeasurer);
    assert!(reports.iter().any(|r| r.max_line_diff() >= 1));
    // A tolerance on the width does not forgive a missing line.
    let wide_open = Tolerance {
        lines: 0,
        ..loose()
    };
    assert!(!failing(&reports, wide_open).is_empty());
}

#[test]
fn an_answer_that_is_not_a_size_fails_whatever_the_tolerance() {
    for candidate in [&NotASize as &dyn TextMeasurer, &HugeNegative, &OverTheCap] {
        let reports = suite(candidate, &StubMeasurer);
        assert_eq!(failing(&reports, loose()).len(), reports.len());
        assert!(reports.iter().all(|r| r.max_width_diff().is_infinite()));
        assert!(reports.iter().all(|r| r.max_line_diff() == u64::MAX));
    }
}

#[test]
fn a_huge_negative_height_does_not_panic_the_line_arithmetic() {
    let reports = suite(&HugeNegative, &StubMeasurer);
    for r in reports.iter().flat_map(|r| &r.results) {
        let _ = r.line_diff();
        assert_eq!(r.candidate_lines, 0);
    }
}

#[test]
fn a_reference_that_answers_no_size_fails_the_probe_and_the_maxima_agree() {
    let reports = suite(&StubMeasurer, &NotASize);
    assert_eq!(failing(&reports, loose()).len(), reports.len());
    assert!(reports.iter().all(|r| r.max_width_diff().is_infinite()));
}

#[test]
fn a_text_outside_the_contract_is_reported_and_never_a_failure() {
    // The URL text of f14 has spaces, so a measurer that never wraps differs from the stub on it; it is outside the
    // contract, so it shows in the results and not in the failures.
    let reports = suite(&NeverWraps, &StubMeasurer);
    let edge = reports
        .iter()
        .find(|r| r.name == "f14-text-edge")
        .expect("f14");
    let differing: Vec<_> = edge
        .results
        .iter()
        .filter(|r| !r.in_contract && r.line_diff() != 0)
        .collect();
    assert!(!differing.is_empty(), "no out-of-contract probe differs");
    let failed = edge.failures(Tolerance::EXACT);
    for d in &differing {
        assert!(
            !failed.iter().any(|f| std::ptr::eq(*f, *d)),
            "an out-of-contract probe was counted: {:?}",
            d.probe.node.text
        );
    }
    assert!(failed.iter().all(|r| r.in_contract));
}

#[test]
fn a_tolerance_lets_a_measurer_that_is_a_little_off_pass() {
    struct OnePixelWider;
    impl TextMeasurer for OnePixelWider {
        fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
            let (w, h) = StubMeasurer.measure(node, wrap_w);
            (w + 1.0, h)
        }
    }
    let reports = suite(&OnePixelWider, &StubMeasurer);
    assert!(!failing(&reports, Tolerance::EXACT).is_empty());
    let one = Tolerance {
        width: 1.0,
        ..Tolerance::EXACT
    };
    assert!(failing(&reports, one).is_empty());
}
