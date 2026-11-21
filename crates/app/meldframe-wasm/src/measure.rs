//! The host's text measurer across the boundary.
//!
//! Layout asks a [`TextMeasurer`] how large a text is; the browser answers with Pixi's `CanvasTextMetrics` on the font
//! the canvas draws with. This module holds the two halves of that: [`GuardedMeasurer`], plain Rust that asks the host a
//! question and refuses an answer that is not a size (so a throwing or confused host can never put `NaN` into the scene),
//! and, on wasm only, [`JsMeasurer`], which asks a JavaScript function.
//!
//! The function the host gives takes `(text: string, fontSize: number, fontWeight: number, wrapWidth: number | null)` and
//! returns `[width, height]`, the same two numbers the contract's measure table is keyed and valued by
//! (`docs/fidelity-contract.md` section 6): the key is the text, the size, the weight and the wrap width, `null` meaning
//! one line with no wrapping; the answer is the width and height in pixels, rounded up. It must not call back into the app.
//! Anything else (an exception, a result that is not two finite numbers from 0 to [`MAX_MEASURE`]) is answered by
//! [`StubMeasurer`] instead: the layout is then provisional, never broken.

use meldframe_layout::{StubMeasurer, TextMeasurer};
use meldframe_protocol::Node;

/// The largest width or height, in pixels, a host answer may have. Past it the answer is taken for a bug (no text a
/// design holds is a million pixels wide), not for a size.
pub const MAX_MEASURE: f64 = 1_000_000.0;

/// What the host is asked: the facts of a text that decide how large it is, and nothing else of the node.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasureQuery<'a> {
    pub text: &'a str,
    pub font_size: f64,
    pub font_weight: u16,
    /// `None`: one line, no wrapping.
    pub wrap_width: Option<f64>,
}

/// Whether `(w, h)` is a size a host may answer with: both numbers finite, from 0 to [`MAX_MEASURE`].
pub fn is_size((w, h): (f64, f64)) -> bool {
    (0.0..=MAX_MEASURE).contains(&w) && (0.0..=MAX_MEASURE).contains(&h)
}

/// A measurer that asks `ask` and falls back to [`StubMeasurer`] when it answers `None` or something that is not a size.
pub struct GuardedMeasurer<F> {
    ask: F,
}

impl<F> std::fmt::Debug for GuardedMeasurer<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuardedMeasurer").finish_non_exhaustive()
    }
}

impl<F: Fn(&MeasureQuery) -> Option<(f64, f64)>> GuardedMeasurer<F> {
    pub fn new(ask: F) -> Self {
        Self { ask }
    }
}

impl<F: Fn(&MeasureQuery) -> Option<(f64, f64)>> TextMeasurer for GuardedMeasurer<F> {
    fn measure(&self, node: &Node, wrap_w: Option<f64>) -> (f64, f64) {
        let query = MeasureQuery {
            text: &node.text,
            font_size: node.font_size,
            font_weight: node.font_weight,
            wrap_width: wrap_w,
        };
        (self.ask)(&query)
            .filter(|size| is_size(*size))
            .unwrap_or_else(|| StubMeasurer.measure(node, wrap_w))
    }
}

/// The measurer a JavaScript function backs. One thread under wasm, so it is not `Send` (see `meldframe_app::MeasurerBox`).
#[cfg(target_arch = "wasm32")]
pub type JsMeasurer = GuardedMeasurer<Box<dyn Fn(&MeasureQuery) -> Option<(f64, f64)>>>;

/// Wraps a JavaScript function as a measurer (the contract is in the module doc).
#[cfg(target_arch = "wasm32")]
pub fn js_measurer(function: js_sys::Function) -> JsMeasurer {
    use wasm_bindgen::{JsCast, JsValue};
    GuardedMeasurer::new(Box::new(move |q: &MeasureQuery| {
        let args = js_sys::Array::of4(
            &JsValue::from_str(q.text),
            &JsValue::from_f64(q.font_size),
            &JsValue::from_f64(f64::from(q.font_weight)),
            &q.wrap_width.map_or(JsValue::NULL, JsValue::from_f64),
        );
        // An exception is an `Err` here, not a trap; whatever comes back is checked as a pair of numbers.
        let answer = function.apply(&JsValue::NULL, &args).ok()?;
        let pair: js_sys::Array = answer.dyn_into().ok()?;
        Some((pair.get(0).as_f64()?, pair.get(1).as_f64()?))
    }))
}
