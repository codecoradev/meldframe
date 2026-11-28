//! The one place of the Rune front end for design values (U4): sizes, the theme, and what Rune's own tokens are
//! called here. A view crate reads these and writes no colour, size or duration of its own. Colours come from Rune's
//! theme (`rune::tok::theme`), so the chrome follows one palette; the colours of the painted file come from the file.

use rune::tok::{Theme, theme};
pub use rune::tok::{radius, space, type_};

/// Whether the chrome is drawn in the dark theme. One value for the whole front end until the app layer carries the
/// person's choice of appearance.
pub const DARK: bool = true;

/// The height of the bar above the canvas, in points.
pub const BAR_HEIGHT: f32 = 40.0;

/// The height of a button of the bar, in points.
pub const BUTTON_HEIGHT: f32 = 28.0;

/// The width of a button of the bar, in points.
pub const BUTTON_WIDTH: f32 = 72.0;

/// The width of the status text, in points.
pub const STATUS_WIDTH: f32 = 160.0;

/// How opaque a control is while it cannot be used.
pub const DISABLED_OPACITY: f32 = 0.4;

/// The colours of the chrome.
pub fn colours() -> Theme {
    theme(DARK)
}
