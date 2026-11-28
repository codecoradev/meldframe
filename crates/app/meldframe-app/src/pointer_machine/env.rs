//! What the pointer machine reads of the app on each event.

use meldframe_app_model::{Camera, Tool};
use meldframe_layout::Rect;

/// What the machine reads of the app on each event.
pub struct Env<'a> {
    pub tool: Tool,
    pub camera: Camera,
    /// The selected ids, as they are now.
    pub selection: &'a [String],
    /// A text edit is open: the press is the field's (it commits when it loses the focus), not the pointer's.
    pub editing: bool,
    /// The app is halted: nothing that writes begins, and selecting and panning still work.
    pub halted: bool,
    /// The id of the front-most node under a world point (`App::hit`, the one rule).
    pub hit: &'a dyn Fn(f64, f64) -> Option<String>,
    /// The box around the selected nodes in world space, none for none (`meldframe_render::extent_of`).
    pub selection_rect: &'a dyn Fn() -> Option<Rect>,
    /// The ids a marquee over a world rectangle selects: the top-level nodes it overlaps (see [`crate::top_level`]), back to front.
    pub within: &'a dyn Fn(Rect) -> Vec<String>,
}

impl std::fmt::Debug for Env<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Env")
            .field("tool", &self.tool)
            .finish_non_exhaustive()
    }
}
