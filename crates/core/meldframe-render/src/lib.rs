//! The render model: nodes in, painted boxes out (E3.1).
//!
//! `resolve` is the one step both consumers are to read: the canvas, then the export (E4.1). It
//! mirrors, for nodes, what the canvas does today in `packages/web-document/src/scene.ts`
//! (`sceneFromNodes` and `resolveScene`); nothing yet proves that the two agree, because the canvas
//! does not read this output. A field that a consumer paints goes through [`PaintedBox`]; the table
//! in [`coverage`] says which node fields each consumer renders and which are marked unsupported,
//! and a test checks the table against what `resolve` paints.

pub mod coverage;
mod resolve;

pub use resolve::{Paint, PaintedBox, PaintedGradient, PaintedImage, resolve};
