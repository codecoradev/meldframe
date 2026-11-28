//! The render model: nodes in, painted boxes out (E3.1).
//!
//! `resolve_with` is the one step both consumers are to read: the canvas, through the wasm bridge
//! (`Bridge::scene_json` hands over the boxes as JSON), then the export (E4.1). Shared fixtures
//! (`tests/fixtures/scene.json`) fix what it paints, and the web tests read the same file. A field
//! that a consumer paints goes through [`PaintedBox`]; the table in [`coverage`] says which node
//! fields each consumer renders and which are marked unsupported, and a test checks the table
//! against what `resolve` paints. A bound field (`{"var": id}`) paints the value its variable has in the mode a
//! [`ModeChoice`] picks (E3.3); `resolve` is `resolve_with` for a file without variables. `resolve_laid_out` is
//! `resolve_with` with the boxes of the layout core (hug, fill, auto layout); the app layer calls it for the scene when the
//! host turns `layout_on_canvas` on, and `resolve_with` otherwise.

mod capture;
pub mod coverage;
mod extent;
mod hit;
mod laid_out;
mod paint_bounds;
mod resolve;

pub use capture::{capture_bounds, capture_bounds_in, capture_rect, capture_rect_in};
pub use extent::{content_extent, extent_of};
pub use hit::{HitBox, HitIndex, contains_point, hit_test, world_rects};
pub use laid_out::resolve_laid_out;
pub use meldframe_variables::ModeChoice;
pub use paint_bounds::PaintBounds;
pub use resolve::{
    Fields, Paint, PaintedBorder, PaintedBox, PaintedGradient, PaintedImage, PaintedShadow,
    resolve, resolve_with,
};
