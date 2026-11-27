//! Conformance of a text measurer: the same questions the layout core asks, put to a measurer under test and to a
//! reference, and the difference reported.
//!
//! A host that measures text with an engine of its own (a native shell with its text engine, a server with fonts) must
//! give the answers the fidelity contract assumes (`docs/fidelity-contract.md` section 6a): the export pins the width
//! and the line height from the measurer, and a browser that opens the export draws the text the way the reference
//! engine does. This module says how far a host is from that, without a browser.
//!
//! How it works. [`run_fixture`] lays a file out with the *reference* measurer and records every question the layout
//! asked ([`collect_probes`]: the text node, so its text, size and weight, and the wrap width the layout worked out from
//! the reference's earlier answers). It then asks the measurer under test the same questions and returns, for each one,
//! the width difference and the line count difference ([`ProbeResult`]). The line count is the height divided by the
//! height of one line of the same font, which is how the export takes its `line-height` (a measured `M`, rule 2 of the
//! contract), so a measurer is not asked for a count it has no way to give.
//!
//! The reference is any [`TextMeasurer`]: a table recorded in a browser (the oracle's `TableMeasurer` is one), or
//! [`StubMeasurer`] for the baseline, against which the stub's own differences are zero. The texts are the oracle's
//! fixtures (`tests/fixtures/oracle_texts.json`, written by `tools/fidelity-oracle/export-text-fixtures.mjs`).
//!
//! ```
//! use meldframe_layout::conformance::{Tolerance, run_fixture};
//! use meldframe_layout::StubMeasurer;
//! use meldframe_protocol::Node;
//!
//! let nodes: Vec<Node> = serde_json::from_str(
//!     r#"[{"id":"t","type":"text","text":"two words","sw":"fill","sh":"hug","w":40}]"#,
//! )
//! .unwrap();
//! let report = run_fixture("one text", &nodes, &StubMeasurer, &StubMeasurer);
//! assert!(report.failures(Tolerance::EXACT).is_empty());
//! ```

use crate::{TextMeasurer, compute_rects, text_outside_contract};
use meldframe_protocol::{Node, NodeKind};
use std::cell::RefCell;
use std::collections::HashSet;

/// One question the layout asked a measurer: a text node (its text, font size and weight) and the wrap width, `None`
/// meaning one line with no wrapping.
#[derive(Debug, Clone)]
pub struct Probe {
    pub node: Node,
    pub wrap_w: Option<f64>,
}

type Seen = (String, u64, u16, Option<u64>);

/// Wraps a measurer and writes down each distinct question it is asked, in the order of asking.
struct Recorder<'a> {
    inner: &'a dyn TextMeasurer,
    seen: RefCell<HashSet<Seen>>,
    probes: RefCell<Vec<Probe>>,
}

impl std::fmt::Debug for Recorder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recorder").finish_non_exhaustive()
    }
}

impl TextMeasurer for Recorder<'_> {
    fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
        let key = (
            node.text.clone(),
            node.font_size.to_bits(),
            node.font_weight,
            wrap_w.map(f64::to_bits),
        );
        if self.seen.borrow_mut().insert(key) {
            self.probes.borrow_mut().push(Probe {
                node: node.clone(),
                wrap_w,
            });
        }
        self.inner.measure(node, wrap_w)
    }
}

/// The questions the layout asks about `nodes` when `reference` answers, each once, in the order of asking.
pub fn collect_probes(nodes: &[Node], reference: &dyn TextMeasurer) -> Vec<Probe> {
    let recorder = Recorder {
        inner: reference,
        seen: RefCell::default(),
        probes: RefCell::default(),
    };
    compute_rects(nodes, &recorder);
    recorder.probes.into_inner()
}

/// The largest differences a measurer may have from the reference. [`Tolerance::EXACT`] is the baseline of a measurer
/// compared with itself; a host picks its own for a different engine (the oracle holds boxes to 0.05 px, so a width
/// that is off by more than that moves a box).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// The largest `|width - reference width|`, in pixels.
    pub width: f64,
    /// The largest `|lines - reference lines|`.
    pub lines: u32,
}

impl Tolerance {
    /// No difference at all.
    pub const EXACT: Tolerance = Tolerance {
        width: 0.0,
        lines: 0,
    };
}

/// What the measurer under test and the reference answered for one [`Probe`].
#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub probe: Probe,
    /// The text is one the contract promises (`false`: a tab, a URL or CJK text, which a browser breaks differently from
    /// the canvas; see `text_outside_contract`). Such a probe is reported and never counted as a failure.
    pub in_contract: bool,
    pub reference: (f64, f64),
    pub candidate: (f64, f64),
    /// The candidate answered two numbers that are finite and not negative (one that is not is never a size a layout
    /// can use; the wasm bridge swaps it for the stub's, a native host has no guard, see [`TextMeasurer`]).
    pub candidate_is_size: bool,
    pub reference_lines: i64,
    pub candidate_lines: i64,
}

impl ProbeResult {
    /// The candidate width minus the reference width, in pixels (NaN when the candidate did not answer a size).
    pub fn width_diff(&self) -> f64 {
        self.candidate.0 - self.reference.0
    }

    /// The candidate line count minus the reference line count.
    pub fn line_diff(&self) -> i64 {
        self.candidate_lines - self.reference_lines
    }

    /// Whether the candidate is within `tolerance` of the reference: an answer that is a size, a width and a line count
    /// no further off than the tolerance says.
    pub fn within(&self, tolerance: Tolerance) -> bool {
        self.candidate_is_size
            && self.width_diff().abs() <= tolerance.width
            && self.line_diff().unsigned_abs() <= u64::from(tolerance.lines)
    }
}

/// The results of one fixture, one for each question the layout asked.
#[derive(Debug, Clone)]
pub struct FixtureReport {
    pub name: String,
    pub results: Vec<ProbeResult>,
}

impl FixtureReport {
    /// The probes in the contract that are further from the reference than `tolerance` allows.
    pub fn failures(&self, tolerance: Tolerance) -> Vec<&ProbeResult> {
        self.results
            .iter()
            .filter(|r| r.in_contract && !r.within(tolerance))
            .collect()
    }

    /// The largest absolute width difference among the probes in the contract (infinite when a candidate did not
    /// answer a size).
    pub fn max_width_diff(&self) -> f64 {
        self.in_contract()
            .map(|r| {
                if r.candidate_is_size {
                    r.width_diff().abs()
                } else {
                    f64::INFINITY
                }
            })
            .fold(0.0, f64::max)
    }

    /// The largest absolute line count difference among the probes in the contract.
    pub fn max_line_diff(&self) -> u64 {
        self.in_contract()
            .map(|r| r.line_diff().unsigned_abs())
            .max()
            .unwrap_or(0)
    }

    fn in_contract(&self) -> impl Iterator<Item = &ProbeResult> {
        self.results.iter().filter(|r| r.in_contract)
    }
}

/// How many lines a text of height `height` is: the height over the height of one line of the same font, the line being
/// a measured `M` as in the export. 0 when the measurer gave no line height to divide by.
fn lines(measurer: &dyn TextMeasurer, node: &Node, height: f64) -> i64 {
    let mut one = node.clone();
    one.text = "M".into();
    let line = measurer.measure(&one, None).1;
    if line.is_finite() && line > 0.0 && height.is_finite() {
        (height / line).round() as i64
    } else {
        0
    }
}

/// Puts the questions the layout asks about `nodes` (one fixture) to `candidate` and to `reference`.
pub fn run_fixture(
    name: &str,
    nodes: &[Node],
    candidate: &dyn TextMeasurer,
    reference: &dyn TextMeasurer,
) -> FixtureReport {
    let results = collect_probes(nodes, reference)
        .into_iter()
        .filter(|p| p.node.kind == NodeKind::Text)
        .map(|probe| {
            let want = reference.measure(&probe.node, probe.wrap_w);
            let got = candidate.measure(&probe.node, probe.wrap_w);
            ProbeResult {
                in_contract: !text_outside_contract(&probe.node.text),
                candidate_is_size: [got.0, got.1].iter().all(|v| v.is_finite() && *v >= 0.0),
                reference_lines: lines(reference, &probe.node, want.1),
                candidate_lines: lines(candidate, &probe.node, got.1),
                reference: want,
                candidate: got,
                probe,
            }
        })
        .collect();
    FixtureReport {
        name: name.to_owned(),
        results,
    }
}
