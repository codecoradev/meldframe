//! The one place for colour, font-size and duration literals (U4).
//!
//! The values are placeholders that make the module and the lint usable now. They are replaced
//! from the design file when it exists, together with the test that compares the two.

/// Text colour, as a CSS colour string.
pub const COLOUR_TEXT: &str = "#1a1a1a";
/// Body font size in pixels.
pub const FONT_SIZE_BODY_PX: f32 = 14.0;
/// Short transition in milliseconds.
pub const DURATION_FAST_MS: u32 = 120;
