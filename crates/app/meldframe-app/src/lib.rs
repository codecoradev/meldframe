//! UI-neutral state, actions, effects and events.
//!
//! A front end (the web editor now; a desktop or mobile app or a test harness later) does three things
//! only: read [`AppState`], send [`Action`]s, and carry out the [`Effect`]s it is given. Every type here is
//! plain serialisable data with no UI toolkit (ADR-0008, standard rule U1).
//!
//! The [`App`] is a synchronous, deterministic state machine: it does no I/O, reads no clock and draws
//! nothing. Time comes in as a payload of an action; the transport is an effect the shell carries out. The
//! document lives in a [`meldframe_sync::Replica`] inside the app: the confirmed file, plus the person's
//! own pending ops, plus their undo history.
//!
//! An [`Action`] is an input to this layer. It is not a [`meldframe_protocol::Command`]: the command is the
//! change to the file, and the app layer works out the commands from what the person meant.

mod action;
mod app;
mod document;
mod effect;
mod event;
mod link;
mod retry;
mod state;

pub use action::{Action, CreateKind, Frame, Property};
pub use app::App;
pub use effect::{Effect, OutOp};
pub use event::Event;
pub use link::{ATTEMPT_MS, SILENT_MS};
pub use meldframe_sync::LoadError;
pub use state::{AppState, Connection, History, Panel, Panels, Properties, Tool};
