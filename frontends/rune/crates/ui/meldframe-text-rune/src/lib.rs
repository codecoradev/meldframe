//! The text measurer of the Rune front end (E8.2.3): Rune's own text engine answering the questions the layout core asks
//! through the [`TextMeasurer`] seam (`meldframe_layout::measure`), so that a file is laid out with the sizes the Rune
//! views will draw. The conformance suite of E8.1.7 (`meldframe_layout::conformance`) says how far it is from the
//! reference of the fidelity contract (`docs/fidelity-contract.md` section 6a); the criterion is C2 of
//! `docs/spikes/rune-desktop-spike.md`.
//!
//! The font is the face Rune bundles (Inter, regular, medium and semibold, not the variable face of the reference, so a weight
//! of 700 or more is what Rune makes of it: the conformance run reports the difference and this crate does not hide it). A
//! front end that draws its canvas and exports with one measurer gives the app this one (the seam, "One measurer for drawing
//! and for export"). The crate holds no state: Rune keeps a text system of its own on each thread.

use meldframe_layout::{MAX_MEASURE, TextMeasurer};
use meldframe_protocol::Node;
use rune::TextStyle;

/// Sizes text with Rune: `rune::text::measure` for the font size and weight of the node and the wrap width it is asked, with
/// the line height pinned to whole pixels, then the rounding up to whole pixels that the seam makes the host's job.
/// [`RuneMeasurer::exact`] is the answer before the rounding up of the width and the height.
///
/// The line height is the contract's pin (`docs/fidelity-contract.md` section 6, rule 2, and the seam doc): the height of one
/// measured `M` of the font, rounded up to a whole pixel. Every line of a text is that tall, so a text of `n` lines is `n` times
/// it, as a browser lays out the exported `line-height`. Rune's own line height, 1.25 times the size, is not a whole number of
/// pixels, and `n` lines of it rounded up once are not `n` times the pin.
///
/// There is no `Default`: a measurer is made by [`RuneMeasurer::bundled_fonts_only`], which fixes the fonts, or by
/// [`RuneMeasurer::with_system_fonts`], which says that it does not.
#[derive(Debug, Clone, Copy)]
pub struct RuneMeasurer;

impl RuneMeasurer {
    /// A measurer that uses the fonts Rune bundles and none of the machine's, so that the numbers are the same on every
    /// machine. It switches the system fonts off for the whole process (`rune::text::set_system_fonts`), as the headless
    /// tests of the desktop shell do.
    pub fn bundled_fonts_only() -> RuneMeasurer {
        rune::text::set_system_fonts(false);
        RuneMeasurer
    }

    /// A measurer that also uses the machine's fonts (an emoji or a CJK fallback, or a font the machine has under the name of
    /// the bundled one), so its numbers depend on the machine. It switches the system fonts on for the whole process. For a
    /// comparison with [`RuneMeasurer::bundled_fonts_only`] and for a front end that wants the fallback, never for a record.
    pub fn with_system_fonts() -> RuneMeasurer {
        rune::text::set_system_fonts(true);
        RuneMeasurer
    }

    /// The size Rune lays the text out at, in pixels, before the rounding up: 0 by 0 for a font size that is not a positive
    /// number, and never above [`MAX_MEASURE`]. An empty text is one space, as the stub and the browser host measure it. A
    /// wrap width that is not a number is no wrapping; a negative one is 0, the widest unbreakable run.
    pub fn exact(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
        let size = node.font_size;
        if !(size.is_finite() && size > 0.0) {
            return (0.0, 0.0);
        }
        let base = TextStyle::new(size.min(MAX_MEASURE) as f32, f32::from(node.font_weight));
        // The pin: one measured `M` with no wrapping, rounded up to a whole pixel, as the export takes its `line-height`.
        let line = rune::text::measure("M", &[], base, None).1.ceil().max(1.0);
        let style = base.lh(line);
        let text = if node.text.is_empty() {
            " "
        } else {
            node.text.as_str()
        };
        let wrap = wrap_w
            .filter(|w| w.is_finite())
            .map(|w| w.clamp(0.0, MAX_MEASURE) as f32);
        let (w, h) = rune::text::measure(text, &[], style, wrap);
        (size_of(f64::from(w)), size_of(f64::from(h)))
    }
}

/// A number as one side of a size: not a number is 0, and the largest a host may answer is the cap.
fn size_of(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, MAX_MEASURE)
    }
}

impl TextMeasurer for RuneMeasurer {
    fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
        let (w, h) = self.exact(node, wrap_w);
        (size_of(w.ceil()), size_of(h.ceil()))
    }
}
