//! Pure parts of the app layer: views, checks and commands that are functions of their arguments and hold no state.
//!
//! Everything here is a function of its arguments (or `PanelKey`, which holds what a view was built from): no state of
//! the app, no events, no I/O and no clock. The state machine that owns the state, the replica and the events is in
//! the rest of `meldframe-app`, and calls these. The rules that hold a little state of their own are in
//! `meldframe-app-rules`.

pub mod answers;
pub mod binding;
pub mod document;
pub mod image;
pub mod layers;
pub mod layout;
pub mod manage;
pub mod manage_form;
pub mod manage_names;
pub mod panel;
pub mod picker;
pub mod properties;
pub mod property;
pub mod selection;
pub mod text_edit;
pub mod touched;
pub mod typed;
pub mod variables_panel;
