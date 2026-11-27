//! Conformance of a text measurer: the same questions the layout core asks, put to a measurer under test and to a
//! reference, and the difference reported.
//!
//! A front end that measures text with an engine of its own must give the answers the fidelity contract assumes
//! (`docs/fidelity-contract.md` section 6a): the export pins the width and the line height from the measurer, and a
//! browser that opens the export draws the text the way the reference engine does. This module says how far a measurer
//! is from that, without a browser.
//!
//! How it works. [`run_fixture`] lays a file out with the *reference* measurer and records every question the layout
//! asked ([`collect_probes`]: the text node, so its text, size and weight, and the wrap width the layout worked out from
//! the reference's earlier answers). It then asks the measurer under test the same questions and returns, for each one,
//! the width difference, the line count difference and the difference in the height of one line ([`ProbeResult`]).
//!
//! The line count is derived: the height over the height of one line of the same font, which is the height of a measured
//! `M` as the export takes its `line-height` (rule 2 of the contract). It is exact only up to rounding (a host that rounds
//! the whole height up, as the browser host does, can come out a line short on a long text), and a measurer whose line
//! height and heights are wrong by the same factor has the right count. That is why the line height is compared on its
//! own, and why [`Tolerance`] has a limit for it.
//!
//! The suite does not check that widths are whole pixels (the contract asks a host to round up, the core does not
//! round); it reports the width difference. A measurer that panics takes the suite down with it.
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
//! // A report with no probe in the contract would pass for nothing.
//! assert!(report.results.iter().any(|r| r.in_contract));
//! assert!(report.failures(Tolerance::EXACT).is_empty());
//! ```

use crate::{TextMeasurer, compute_rects, is_size, text_outside_contract};
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
/// compared with itself; a front end picks its own for a different engine (the oracle holds boxes to 0.05 px, so a
/// width that is off by more than that moves a box).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// The largest `|width - reference width|`, in pixels.
    pub width: f64,
    /// The largest `|lines - reference lines|`.
    pub lines: u32,
    /// The largest `|height of one line - reference height of one line|`, in pixels.
    pub line_height: f64,
}

impl Tolerance {
    /// No difference at all.
    pub const EXACT: Tolerance = Tolerance {
        width: 0.0,
        lines: 0,
        line_height: 0.0,
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
    /// The height of one line of the probe's font (a measured `M`, no wrapping).
    pub reference_line_height: f64,
    pub candidate_line_height: f64,
    /// The candidate answered sizes (see [`is_size`]: finite, from 0 to `MAX_MEASURE`) for the question and for its line
    /// height. One that did not is never usable in a layout: the browser bridge refuses it, an app with a measurer of its
    /// own does not.
    pub candidate_is_size: bool,
    /// The same for the reference. A reference that answers no size makes the probe fail: nothing can be compared with it.
    pub reference_is_size: bool,
    /// The lines of the text, derived from the height and the height of one line (0 where either is not a size).
    pub reference_lines: i64,
    pub candidate_lines: i64,
}

impl ProbeResult {
    /// Both sides answered sizes, so the differences below mean something.
    pub fn comparable(&self) -> bool {
        self.candidate_is_size && self.reference_is_size
    }

    /// The candidate width minus the reference width, in pixels (NaN when either did not answer a size).
    pub fn width_diff(&self) -> f64 {
        if self.comparable() {
            self.candidate.0 - self.reference.0
        } else {
            f64::NAN
        }
    }

    /// The candidate line count minus the reference line count.
    pub fn line_diff(&self) -> i64 {
        self.candidate_lines.saturating_sub(self.reference_lines)
    }

    /// The candidate height of one line minus the reference's, in pixels (NaN when either did not answer a size).
    pub fn line_height_diff(&self) -> f64 {
        if self.comparable() {
            self.candidate_line_height - self.reference_line_height
        } else {
            f64::NAN
        }
    }

    /// Whether the candidate is within `tolerance` of the reference: both answered sizes, and the width, the line count
    /// and the height of one line are no further off than the tolerance says.
    pub fn within(&self, tolerance: Tolerance) -> bool {
        self.comparable()
            && self.width_diff().abs() <= tolerance.width
            && self.line_diff().unsigned_abs() <= u64::from(tolerance.lines)
            && self.line_height_diff().abs() <= tolerance.line_height
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
        self.in_contract()
            .filter(|r| !r.within(tolerance))
            .collect()
    }

    /// The largest absolute width difference among the probes in the contract (infinite when a side did not answer a
    /// size).
    pub fn max_width_diff(&self) -> f64 {
        self.largest(|r| r.width_diff().abs())
    }

    /// The largest absolute difference of the height of one line among the probes in the contract (infinite when a side
    /// did not answer a size).
    pub fn max_line_height_diff(&self) -> f64 {
        self.largest(|r| r.line_height_diff().abs())
    }

    /// The largest absolute line count difference among the probes in the contract (`u64::MAX` when a side did not
    /// answer a size).
    pub fn max_line_diff(&self) -> u64 {
        self.in_contract()
            .map(|r| {
                if r.comparable() {
                    r.line_diff().unsigned_abs()
                } else {
                    u64::MAX
                }
            })
            .max()
            .unwrap_or(0)
    }

    fn largest(&self, diff: impl Fn(&ProbeResult) -> f64) -> f64 {
        self.in_contract()
            .map(|r| {
                if r.comparable() {
                    diff(r)
                } else {
                    f64::INFINITY
                }
            })
            .fold(0.0, f64::max)
    }

    fn in_contract(&self) -> impl Iterator<Item = &ProbeResult> {
        self.results.iter().filter(|r| r.in_contract)
    }
}

/// The height of one line of the node's font: a measured `M` with no wrapping, as the export takes its `line-height`.
fn line_height(measurer: &dyn TextMeasurer, node: &Node) -> (f64, f64) {
    let mut one = node.clone();
    one.text = "M".into();
    measurer.measure(&one, None)
}

/// How many lines a text of height `height` is, given the height of one line: 0 where either is not a size or the line
/// has no height. The division is exact only up to the host's rounding (a long text whose whole height is rounded up
/// can come out a line short).
fn lines(line: (f64, f64), height: f64) -> i64 {
    if is_size(line) && line.1 > 0.0 && height.is_finite() && height >= 0.0 {
        (height / line.1).round() as i64
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
            let want_line = line_height(reference, &probe.node);
            let got_line = line_height(candidate, &probe.node);
            ProbeResult {
                in_contract: !text_outside_contract(&probe.node.text),
                candidate_is_size: is_size(got) && is_size(got_line),
                reference_is_size: is_size(want) && is_size(want_line),
                reference_lines: lines(want_line, want.1),
                candidate_lines: lines(got_line, got.1),
                reference_line_height: want_line.1,
                candidate_line_height: got_line.1,
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
