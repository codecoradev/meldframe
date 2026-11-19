//! The render model: nodes in, painted boxes out (E3.1).
//!
//! `resolve` is the one step both consumers read: the canvas now, the export (E4.1) later. A field
//! that a consumer paints goes through [`PaintedBox`]; the table in [`coverage`] says which node
//! fields each consumer renders and which are marked unsupported, and a test keeps it true.

pub mod coverage;
mod resolve;

pub use resolve::{Paint, PaintedBox, PaintedGradient, PaintedImage, resolve};
