//! The pointer state machine of the canvas (E8.1.4, part 2): what a press, a drag, a release, a cancel and a hover come to, as a
//! pure function of the machine's own mode and of what the app tells it ([`Env`]).
//!
//! It holds no file, no scene and no
//! selection: the one rule for what is under a point is asked through [`Env::hit`] (`App::hit`, T9), the geometry of a drag that
//! moves, sizes or draws a node is the rules of `drop`, `multi`, `draw` and `handle`, the handles are `canvas_overlay`, and a
//! gesture comes out as the actions the file already has (`Action::Select`, `Action::DropNodes`, `Action::Gesture`,
//! `Action::Camera`). The state it shows is `PointerState`.

mod env;
mod machine;
mod marquee;
mod preview;
mod screen;
mod thresholds;

pub use env::Env;
pub use machine::Machine;
pub use marquee::top_level;
pub use preview::{PointerPreview, Queries, preview};
pub use thresholds::{DOUBLE_CLICK_MS, DOUBLE_CLICK_PX, DRAG_THRESHOLD};
