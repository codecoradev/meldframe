//! The conformance suite of a text measurer (E8.1.7): the stub passes its own baseline over the texts of the oracle
//! fixtures, and measurers that are wrong in a known way fail it.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_layout::conformance::{FixtureReport, Tolerance, run_fixture};
use meldframe_layout::{StubMeasurer, TextMeasurer};
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

/// Wider glyphs than the stub (0.6 em against 0.5 em, the stub's rule at a font size 1.2 times larger), one line height.
struct WideGlyphs;

impl TextMeasurer for WideGlyphs {
    fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
        let mut wide = node.clone();
        wide.font_size = node.font_size * 1.2;
        let (w, _) = StubMeasurer.measure(&wide, wrap_w);
        (w, StubMeasurer.measure(node, None).1)
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
    }
}

#[test]
fn a_measurer_with_wider_glyphs_fails_on_the_width() {
    let reports = suite(&WideGlyphs, &StubMeasurer);
    let failed = failing(&reports, Tolerance::EXACT);
    assert!(failed.len() >= 5, "only {failed:?} failed");
    assert!(reports.iter().any(|r| r.max_width_diff() > 1.0));
}

#[test]
fn a_measurer_that_never_wraps_fails_on_the_line_count_and_not_on_a_loose_width_tolerance() {
    let reports = suite(&NeverWraps, &StubMeasurer);
    assert!(reports.iter().any(|r| r.max_line_diff() >= 1));
    // A tolerance on the width does not forgive a missing line.
    let wide_open = Tolerance {
        width: 1.0e9,
        lines: 0,
    };
    assert!(!failing(&reports, wide_open).is_empty());
}

#[test]
fn an_answer_that_is_not_a_size_fails_whatever_the_tolerance() {
    let reports = suite(&NotASize, &StubMeasurer);
    let loose = Tolerance {
        width: 1.0e9,
        lines: 1000,
    };
    assert_eq!(failing(&reports, loose).len(), reports.len());
    assert!(reports.iter().all(|r| r.max_width_diff().is_infinite()));
}

#[test]
fn a_text_outside_the_contract_is_reported_and_never_a_failure() {
    // A measurer that never wraps differs on the wrapped texts of f14, but the tabs, URLs and CJK texts of that
    // fixture are not counted.
    let reports = suite(&NeverWraps, &StubMeasurer);
    let edge = reports
        .iter()
        .find(|r| r.name == "f14-text-edge")
        .expect("f14");
    assert!(edge.results.iter().any(|r| !r.in_contract));
    assert!(
        edge.failures(Tolerance::EXACT)
            .iter()
            .all(|r| r.in_contract),
        "an out-of-contract probe was counted"
    );
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
    assert!(
        failing(
            &reports,
            Tolerance {
                width: 1.0,
                lines: 0
            }
        )
        .is_empty()
    );
}
