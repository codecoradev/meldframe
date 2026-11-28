//! What the server hands the routes: who may make a row, the policies, the publisher of the frame and the bounds of the state route.

use crate::state::Past;
use meldframe_host::Principal;
use meldframe_module::Policy;
use meldframe_protocol::ServerMsg;
use std::sync::Arc;

/// Queues a frame on the live streams of a file of the principal's workspace, as ephemeral and with headroom; answers
/// how many streams took it. The server's `SharedServer::publish_with_headroom` is the one.
pub type Publish = Arc<dyn Fn(&Principal, &str, ServerMsg) -> usize + Send + Sync>;

/// Whether an actor may make a checkpoint: until a principal has a kind, the listed reviewers of ADR-0021 (the binary
/// builds the closure from `MELDFRAME_REVIEWERS`, so this crate does not depend on the activity one).
pub type May = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// What the server hands the routes: who may make a row or read a file at one, what the registry's policy for
/// [`CREATE_TOOL`] and for [`STATE_TOOL`] is (each call is made only under `Auto`), the publisher of the frame and the
/// reader of the log the state route replays.
#[derive(Clone)]
pub struct Gate {
    pub may: May,
    pub policy: Option<Policy>,
    pub state_policy: Option<Policy>,
    pub publish: Publish,
    pub past: Arc<dyn Past>,
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gate")
            .field("policy", &self.policy)
            .field("state_policy", &self.state_policy)
            .finish_non_exhaustive()
    }
}

/// The bounds of the state route: how many ops a replay may read (the snapshot route's window), and how many answers the
/// cache holds and how many bytes of their text (ADR-0023; a test gives smaller ones).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateLimits {
    pub replay: u64,
    pub entries: usize,
    pub bytes: usize,
}

impl Default for StateLimits {
    fn default() -> Self {
        StateLimits {
            replay: 5_000,
            entries: 64,
            bytes: 16 * 1024 * 1024,
        }
    }
}

/// The server clock in milliseconds.
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;
