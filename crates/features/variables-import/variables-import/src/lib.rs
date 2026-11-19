//! Variables import feature module (E3.4): CSS custom properties and Tailwind v4 `@theme` text to the commands that add
//! collections, modes, variables and aliases to a file (ADR-0020), with a report of what was converted or skipped.
//!
//! [`import_css`] is a pure function from text to commands. It runs no code and opens nothing: `@import` and `url()`
//! are reported as skipped and never followed, a value that is computed (`calc()`, `color-mix()`) is skipped and never
//! evaluated, and the size of the text and the number of what it makes are bounded. The commands are the ones of
//! `meldframe-protocol`, in an order in which each can be applied after the ones before it (collections first, an
//! alias after the variable it points at), so the caller can send them as one atomic batch through the same gate as any
//! other command.
//!
//! What it makes: `@theme` variables go to the collection `theme` (one mode, `default`); custom properties of `:root`
//! go to the collection `root`, whose modes are `light` (the root rule) and `dark` (a rule that picks the dark mode,
//! such as `.dark`, `[data-theme=dark]` or a `prefers-color-scheme: dark` query); a pure `var(--x)` value is an alias.
//! A colour that is not hex is converted to hex and the report says so; one that sRGB cannot show is clamped and the
//! report says that too. Nothing is changed without a line in the report.

pub mod color;
pub mod scan;
mod text;
pub mod value;

/// The bytes of text the report helpers have cut (a test reads it; nothing else does).
#[doc(hidden)]
pub static TEXT_WORK: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
