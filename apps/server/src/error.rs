//! Why a submit wrote nothing, or stopped part way.

use meldframe_host::{Refusal, RoomError};
use meldframe_rooms::RegistryError;
use std::fmt;

/// Why `Server::submit_tool` wrote nothing, or stopped part way.
#[derive(Debug, Clone, PartialEq)]
pub enum SubmitError {
    /// The tool or the gate refused the batch; nothing was written.
    Refused(Refusal),
    /// The file would not open, or the room refused or failed an op. Ops of the same batch that came before
    /// stay written (each op is its own durable write); sending the batch again with the same keys returns
    /// those as `written: false` and writes only the rest.
    Rooms(RegistryError),
}

impl fmt::Display for SubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubmitError::Refused(r) => write!(f, "{r}"),
            SubmitError::Rooms(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SubmitError {}

impl SubmitError {
    /// The refusal behind this error, whether the tool, the gate or the room refused: one shape (and one
    /// stable `Refusal::code`) for a transport. `None` for a failure that is not a refusal (the store is
    /// down, the registry is full, the log does not replay).
    pub fn refusal(&self) -> Option<&Refusal> {
        match self {
            SubmitError::Refused(r)
            | SubmitError::Rooms(RegistryError::Room(RoomError::Refused(r))) => Some(r),
            SubmitError::Rooms(_) => None,
        }
    }
}

impl From<Refusal> for SubmitError {
    fn from(r: Refusal) -> Self {
        SubmitError::Refused(r)
    }
}

impl From<RegistryError> for SubmitError {
    fn from(e: RegistryError) -> Self {
        SubmitError::Rooms(e)
    }
}
