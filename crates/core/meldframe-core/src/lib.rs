//! Document model, commands, history and the render model.

pub mod layout;
pub use layout::{Rect, StubMeasurer, TextMeasurer, compute_rects, compute_rects_indexed};
