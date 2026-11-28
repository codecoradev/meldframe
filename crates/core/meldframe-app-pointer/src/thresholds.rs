//! The distances and the time that tell a click, a double click and a drag apart.

/// Pointer travel (screen pixels) before a press becomes a drag.
pub const DRAG_THRESHOLD: f64 = 3.0;

/// The longest time between two presses that make a double click, in milliseconds.
pub const DOUBLE_CLICK_MS: u64 = 500;

/// The farthest two presses of a double click may be from each other, in screen pixels.
pub const DOUBLE_CLICK_PX: f64 = 4.0;
