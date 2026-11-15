//! `FileState`: the in-memory tree of one file and the undo and redo history of the local actor.

mod file_state;
mod history;
mod inverse;
mod remote;

pub use file_state::FileState;
pub use meldframe_core::{ApplyError, Limits};
