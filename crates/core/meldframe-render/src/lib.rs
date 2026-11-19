//! The render model: nodes in, painted boxes out (E3.1).
//!
//! `resolve` is the one step both consumers are to read: the canvas, through the wasm bridge
//! (`Bridge::scene_json` hands over the boxes as JSON), then the export (E4.1). Shared fixtures
//! (`tests/fixtures/scene.json`) fix what it paints, and the web tests read the same file. A field
//! that a consumer paints goes through [`PaintedBox`]; the table in [`coverage`] says which node
//! fields each consumer renders and which are marked unsupported, and a test checks the table
//! against what `resolve` paints.

pub mod coverage;
mod resolve;

pub use resolve::{Paint, PaintedBox, PaintedGradient, PaintedImage, resolve};
