//! Pure parts of the app layer, split from `meldframe-app` to keep it under the size ceiling (2,000 lines a crate).
//!
//! Everything here is a function of its arguments (or `PanelKey`, which holds what a view was built from): no state of
//! the app, no events, no I/O and no clock. The state machine that owns the state, the replica and the events stays in
//! `meldframe-app`, and calls these. The rules that hold a little state of their own are in `meldframe-app-rules`.

pub mod binding;
pub mod document;
pub mod layout;
pub mod panel;
pub mod picker;
pub mod properties;
pub mod property;
pub mod selection;
pub mod touched;
pub mod variables_panel;
