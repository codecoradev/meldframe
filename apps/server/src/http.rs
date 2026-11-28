//! The server behind the HTTP adapter: [`SharedServer`] is the one `Submitter`, so REST and WebSocket reach a room only
//! through `Server::submit_tool` (the gate).

mod failure;
/// The reads of the state route of a checkpoint (ADR-0023).
#[cfg(feature = "checkpoints")]
mod past;
#[cfg(feature = "mcp")]
mod read;
mod shared;
mod snapshots;
mod stream;
mod submit;

pub use failure::failure_of;
pub use shared::SharedServer;
