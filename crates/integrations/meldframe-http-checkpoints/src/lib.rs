//! The server side of checkpoints over HTTP (E5.3, ADR-0023): the run sink, the routes, the live frame and the entries
//! of the two calls in the registry.
//!
//! [`install`] is the one call the binary makes: it sets the [`RunSink`] on `meldframe_http::Shared`, the place every
//! write passes (REST, the submit socket and MCP end in `Shared::submit_ops`), so an agent's run start takes a row with
//! no editor open.
//!
//! The detector is [`RunWatch`] of `meldframe-checkpoints`: only an actor with the `agent:` prefix starts a run, the
//! first write after a gap of more than 5 minutes (or the first seen, also after a restart) takes a `runStart` row at the
//! `seq` just before the first op the call wrote. A call that failed after it wrote a prefix is found by key through
//! `Reader::written`, the unlogged read the batch sink uses too. The common path costs a prefix test; only a due run
//! start runs a job, its own job under the caller's slot, so it never holds the call's own job. The reply waits for it
//! for at most the call's time limit (as for the batch sink). A write the store refuses, a job that cannot run or one
//! that outlasts the limit is logged and counted ([`Checkpoints::failures`]), gives the claim back so the next call of
//! the run can take the run start, and never changes the reply; a job that outlasts the limit may still finish, which
//! costs one extra row.
//!
//! A row that was made, a run start or a person's, is queued as a `checkpoint` frame on the live streams of the file
//! (ephemeral, with headroom, through the publisher of [`Gate`]); the routes in `routes.rs` list the rows and let a
//! listed person make one. [`CheckpointsModule`] registers the two calls of a person, `checkpoint_create` and
//! `checkpoint_state`, so a deployment can deny them and the call log records their tier; neither is an MCP tool.
//! The route of `checkpoint_state` (`state.rs`) answers a file as it was at a checkpoint, by a bounded replay, so a
//! restore still works after a reload.

mod gate;
mod install;
mod module;
mod routes;
mod sink;
mod state;
mod wire;

pub use gate::{Clock, Gate, May, Publish, StateLimits};
pub use install::{install, install_with};
pub use module::{
    CREATE_DESCRIPTION, CREATE_TOOL, CheckpointsModule, STATE_DESCRIPTION, STATE_TOOL,
};
pub use sink::Checkpoints;
pub use state::{Log, Past, SCAN_COST};
