//! UI-neutral state, actions, effects and events.
//!
//! A front end (the web editor now; a desktop or mobile app or a test harness later) does three things
//! only: read [`AppState`], send [`Action`]s, and carry out the [`Effect`]s it is given. Every type here is
//! plain serialisable data with no UI toolkit (ADR-0008, standard rule U1).
//!
//! An [`Action`] is an input to this layer. It is not a [`meldframe_protocol::Command`]: the command is the
//! change to the file, and the app layer only wraps one in [`Action::Submit`].

mod action;
mod effect;
mod event;
mod state;

pub use action::{Action, Outcome};
pub use effect::Effect;
pub use event::Event;
pub use state::{AppState, Connection, History, Panel, Panels, Tool};
