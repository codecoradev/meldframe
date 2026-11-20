//! Pure parts of the app layer, split from `meldframe-app` to keep it under the size ceiling (2,000 lines a crate).
//!
//! Everything here is a function of its arguments: no state, no events, no I/O and no clock. The state machine
//! that owns the state, the replica and the events stays in `meldframe-app`, and calls these.

pub mod document;
pub mod picker;
pub mod properties;
pub mod property;
pub mod touched;
pub mod variables_panel;
