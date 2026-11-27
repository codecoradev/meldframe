//! The text measurer seam. Layout asks it how large a text node is; adapters answer.
//!
//! The browser adapter measures with the canvas, a recorded table replays measurements for
//! non-browser targets, and [`StubMeasurer`] gives tests a deterministic answer. A host that
//! brings its own engine (a native shell with its text engine) implements [`TextMeasurer`] and
//! checks it with the suite in [`crate::conformance`]; the contract it has to meet is on the trait.

use meldframe_protocol::Node;

/// Measures text for the font, size, weight and wrap width a node carries.
///
/// # What a host returns
///
/// The question is a text node and a wrap width. The facts that decide the answer are `node.text`, `node.font_size`
/// (pixels) and `node.font_weight` (100 to 900); a node has no font family field, because the fidelity contract has one
/// family, Inter Variable (`docs/fidelity-contract.md` section 6), which the host must measure with. Nothing else of the
/// node may change the answer.
///
/// The answer is `(width, height)` in pixels, both finite and not negative:
///
/// - `wrap_w = None`: the text on one line with no wrapping. The width is the width of the whole text, rounded up to a
///   whole pixel (the core uses `ceil` of the engine's width; the export pins `min-width` to it). The height is the
///   height of one line.
/// - `wrap_w = Some(w)`: the text broken to fit `w` pixels. The width is the width of the widest line (rounded up), the
///   height is the number of lines times the height of one line. Lines break at a space or an explicit newline and
///   nowhere else (section 6, rule 7); a word wider than `w` stays whole on a line of its own. (The stub breaks at
///   spaces only and counts a newline as one character: it is an estimate, not the contract.) A wrap width the host
///   keys a cache on is quantised to 1/64 px (rule 5).
///
/// The height of one line is also asked on its own: the export takes the `line-height` of every text from a measure of a
/// single `M` of the same font with no wrapping, so a line is the same height wherever it is measured and a count of
/// lines is the height over that.
///
/// # What happens to an answer that is not a size
///
/// The trait does not check an answer; each host decides. The browser bridge (`meldframe-wasm`, `setMeasurer`) wraps the
/// JavaScript function in a guard: an exception, a result that is not a pair of numbers, or a width or height that is
/// not finite or is outside 0 to 1,000,000 is answered by [`StubMeasurer`] for that question, so the layout is
/// provisional and never broken. A host that hands `App::set_measurer` a measurer of its own gets no guard from the app:
/// a `NaN` it returns goes into the boxes. It must give only sizes (the conformance suite reports an answer that is not
/// one). Where a host gives no measurer at all, the stub measures.
///
/// # One measurer for drawing and for export
///
/// The core and the export agree on text because both use the one measurer they are given: layout sizes the boxes with
/// it and the export pins `min-width` and `line-height` from it, so the browser that opens the export draws the text the
/// way the engine behind the measurer does. A client must therefore draw its canvas, and export from its Export dialog,
/// with the measurer it gives the app. A canvas that measures with one engine and an export that pins another's
/// numbers breaks the fidelity contract. The MCP `export_frame` tool runs on the server with [`StubMeasurer`] and says so
/// (`text_size_estimated`).
pub trait TextMeasurer {
    /// Returns `(width, height)` in pixels. `wrap_w = None` means one line with no wrapping.
    fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64);
}

/// A deterministic measurer: character width is `font_size * 0.5`, line height is
/// `ceil(font_size * 1.2)`. It breaks lines at spaces only and ignores the font weight.
/// Its outputs are the expected values in the golden layout fixtures.
#[derive(Debug, Clone, Copy, Default)]
pub struct StubMeasurer;

impl TextMeasurer for StubMeasurer {
    fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
        let cw = node.font_size * 0.5;
        let lh = (node.font_size * 1.2).ceil();
        let text: &str = if node.text.is_empty() {
            " "
        } else {
            &node.text
        };
        match wrap_w {
            None => ((text.chars().count() as f64 * cw).ceil(), lh),
            Some(ww) => {
                let mut lines: Vec<String> = Vec::new();
                let mut cur = String::new();
                for word in text.split(' ') {
                    let cand = if cur.is_empty() {
                        word.to_string()
                    } else {
                        format!("{cur} {word}")
                    };
                    if cur.is_empty() || cand.chars().count() as f64 * cw <= ww {
                        cur = cand;
                    } else {
                        lines.push(std::mem::take(&mut cur));
                        cur = word.to_string();
                    }
                }
                lines.push(cur);
                let widest = lines
                    .iter()
                    .map(|l| l.chars().count() as f64 * cw)
                    .fold(f64::NEG_INFINITY, f64::max);
                (widest.ceil(), lines.len() as f64 * lh)
            }
        }
    }
}
