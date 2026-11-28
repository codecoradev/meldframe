//! The seam to the admission gate: [`Submitter`], the one way in to a write.

use crate::reply::Failure;
use meldframe_batch::Guard;
use meldframe_host::{Accepted, Op, Principal};

/// The one way in to a write (the server's `Server::submit_tool`); it blocks, so it runs on `spawn_blocking`.
pub trait Submitter: Send + Sync {
    fn submit(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
    ) -> Result<Vec<Accepted>, Failure>;

    /// Like [`Submitter::submit`], but all or nothing as far as the gate and the room can tell beforehand: if any op
    /// would be refused, none is written and the failure names it ([`Failure::Batch`]). MCP's `apply_commands` uses it.
    fn submit_atomic(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
        guards: Vec<Guard>,
    ) -> Result<Vec<Accepted>, Failure>;
}
