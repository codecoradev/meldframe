//! The seam `apply_commands` keeps a batch header through: what it tells the sink, and what the sink answers.

use crate::Note;

/// What `apply_commands` tells the [`BatchSink`] about a call (ADR-0021). The sink is told the workspace and the actor of the token.
#[derive(Debug)]
pub struct BatchCall {
    pub file: String,
    /// The idempotency key: the ops of the call are `{key}:{i}`.
    pub key: String,
    pub commands: usize,
    pub atomic: bool,
    pub note: Option<Note>,
}

/// The batch a header was kept for: its number, how many of the call's keys are in the log, and what became of the
/// call's note (`kept`, `kept_earlier` or `none`).
#[derive(Debug)]
pub struct Marked {
    pub n: u32,
    pub written: u32,
    pub note: &'static str,
}

/// Keeps the header of a batch after `apply_commands` ran. It blocks, so it runs on the blocking pool. `done` is the
/// `(seq, written)` of each command of a call that succeeded; `None` is a call that failed, and the sink finds out how
/// much of it was written. `None` back: no header was kept, and the call is never failed by it.
pub trait BatchSink: std::fmt::Debug + Send + Sync {
    fn record(
        &self,
        workspace: &str,
        actor: &str,
        call: &BatchCall,
        done: Option<&[(u64, bool)]>,
    ) -> Option<Marked>;
}
