//! Pure parts of the app layer: views, checks and commands that are functions of their arguments and hold no state.
//!
//! Everything here is a function of its arguments (or `PanelKey`, which holds what a view was built from): no state of
//! the app, no events, no I/O and no clock. The state machine that owns the state, the replica and the events is in
//! the rest of `meldframe-app`, and calls these. The modules that other modules of this directory use are still in
//! the crate `meldframe-app-parts`, which this module re-exports until they move (B4, #315).

pub mod answers;
pub mod binding;
pub mod image;
pub mod layers;
pub mod manage;
pub mod manage_form;
pub mod picker;
pub mod selection;
pub mod text_edit;
pub mod touched;
pub mod typed;

pub use meldframe_app_parts::{
    document, layout, manage_names, panel, properties, property, variables_panel,
};
