//! The pointer state machine of the canvas (E8.1.4, part 2): what a press, a drag, a release, a cancel and a hover come to, as a
//! pure function of the machine's own mode and of what the app tells it ([`Env`]). It holds no file, no scene and no
//! selection: the one rule for what is under a point is asked through [`Env::hit`] (`App::hit`, T9), the geometry of a drag that
//! moves, sizes or draws a node is the rules of `drop`, `multi`, `draw` and `handle`, the handles are `canvas_overlay`, and a
//! gesture comes out as the actions the file already has (`Action::Select`, `Action::DropNodes`, `Action::Gesture`,
//! `Action::Camera`). The state it shows is `PointerState`.

mod machine;
mod marquee;
mod preview;

pub use machine::Machine;
pub use marquee::top_level;
pub use preview::{PointerPreview, Queries, preview};

use meldframe_app_model::{Camera, Tool};
use meldframe_layout::Rect;

/// Pointer travel (screen pixels) before a press becomes a drag.
pub const DRAG_THRESHOLD: f64 = 3.0;

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
    /// The ids a marquee over a world rectangle selects: the top-level nodes it overlaps (see [`top_level`]), back to front.
    pub within: &'a dyn Fn(Rect) -> Vec<String>,
}

impl std::fmt::Debug for Env<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Env")
            .field("tool", &self.tool)
            .finish_non_exhaustive()
    }
}
