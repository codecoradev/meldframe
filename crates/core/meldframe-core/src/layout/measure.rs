//! The text measurer seam. Layout asks it how large a text node is; adapters answer.
//!
//! The browser adapter measures with the canvas, a recorded table replays measurements for
//! non-browser targets, and [`StubMeasurer`] gives tests a deterministic answer.

use meldframe_protocol::Node;

/// Measures text for the font, size, weight and wrap width a node carries.
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
