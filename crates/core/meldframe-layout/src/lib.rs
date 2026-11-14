//! Layout: the box of every node, from hug, fill and auto layout rules.
//!
//! The layout result is the source of truth for geometry (ADR-0003); export translates it to CSS.
//! Text sizes come through the [`TextMeasurer`] seam so the crate stays free of I/O.

mod measure;
mod rect;

pub use measure::{StubMeasurer, TextMeasurer};
pub use rect::Rect;
