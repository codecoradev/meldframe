//! The text measurer seam. Layout asks it how large a text node is; adapters answer.
//!
//! The browser adapter measures with the canvas, a recorded table replays measurements for
//! non-browser targets, and [`StubMeasurer`] gives tests a deterministic answer. A front end that
//! brings its own text engine implements [`TextMeasurer`] and checks it with the suite in
//! [`crate::conformance`]; the contract it has to meet is on the trait.

use meldframe_protocol::Node;

/// The largest width or height, in pixels, a host answer may have. Past it the answer is taken for a bug (no text a
/// design holds is a million pixels wide), not for a size.
pub const MAX_MEASURE: f64 = 1_000_000.0;

/// Whether `(w, h)` is a size a host may answer with: both numbers finite, from 0 to [`MAX_MEASURE`].
pub fn is_size((w, h): (f64, f64)) -> bool {
    (0.0..=MAX_MEASURE).contains(&w) && (0.0..=MAX_MEASURE).contains(&h)
}

/// Measures text for the font, size, weight and wrap width a node carries.
///
/// # What a host returns
///
/// The question is a text node and a wrap width. The facts that decide the answer are `node.text`, `node.font_size`
/// (pixels) and `node.font_weight` (100 to 900); a node has no font family field, because the fidelity contract has one
/// family, Inter Variable (`docs/fidelity-contract.md` section 6), which the host must measure with. Nothing else of the
/// node may change the answer. An empty text measures as one space (the stub and the browser host do).
///
/// The answer is `(width, height)` in pixels, each finite and from 0 to [`MAX_MEASURE`] (see [`is_size`]):
///
/// - `wrap_w = None`: the text on one line with no wrapping. The width is the width of the whole text and the height is
///   the height of one line.
/// - `wrap_w = Some(w)`: the text broken to fit `w` pixels. The width is the width of the widest line and the height is
///   about the number of lines times the height of one line, up to the host's rounding (the browser host rounds the whole
///   height up, not each line). Lines break at a space, an explicit newline or a hyphen; nothing else is promised
///   (section 6, rule 7, and the oracle's hyphen case). A word wider than `w` stays whole on a line of its own. The stub
///   breaks at spaces only and counts a newline as one character: it is an estimate, not the contract.
///
/// **Rounding is the host's job.** The layout core takes the width and height as they are (`size.rs` does not round), so
/// the host must round them up to whole pixels, as the stub and the browser host do (`Math.ceil` of Pixi's width and
/// height); the export rounds the width up again when it pins `min-width`. A host that answers fractional widths gets
/// fractional boxes. The conformance suite does not check that widths are whole numbers; it reports the width difference,
/// so an unrounded answer shows as a difference of under a pixel unless the tolerance forgives it.
///
/// A wrap width a host keys a cache on is quantised to 1/64 px (rule 5). The height of one line is also asked on its own:
/// the export takes the `line-height` of every text from a measure of a single `M` of the same font with no wrapping.
///
/// # What happens to an answer that is not a size
///
/// The trait does not check an answer; each host decides. The browser bridge (`meldframe-wasm`, `setMeasurer`) takes the
/// answer of the JavaScript function through [`is_size`]: an exception, a result that is not an array (a longer array
/// is read at its first two elements), or a width or height that is not finite or is outside 0 to [`MAX_MEASURE`] is
/// refused for that question and [`StubMeasurer`] answers it, so the layout is provisional and never broken. A front end
/// that hands `App::set_measurer` a measurer of its own gets no such refusal from the app: a `NaN` it returns goes into
/// the boxes, so it must give only sizes (the conformance suite reports an answer that is not one). Where a host gives no
/// measurer at all, the stub measures. A measurer that panics takes the layout call down with it. On a native target the
/// app takes a `Send` measurer (`MeasurerBox`); under WASM it does not need to be.
///
/// # One measurer for drawing and for export
///
/// The core and the export agree on text because both use the one measurer they are given: layout sizes the boxes with
/// it and the export pins `min-width` and `line-height` from it, so the browser that opens the export draws the text the
/// way the engine behind the measurer does. A front end must therefore draw its canvas, and export from its Export
/// dialog, with the measurer it gives the app. A canvas that measures with one engine and an export that pins another's
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
