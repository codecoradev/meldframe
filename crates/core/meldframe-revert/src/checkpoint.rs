//! When a checkpoint is due on its own (E5.3): at the start of an agent run.
//!
//! The log carries no time, so the caller gives each op's clock value (the app layer gets it as the payload of
//! the action that delivers the op). A run is a stretch of ops by one actor with no pause longer than the gap.

use std::collections::HashMap;

/// A run ends when its actor is quiet for more than this (5 minutes). The lane's run (`meldframe-activity`), the
/// checkpoint the app layer took and the one the server takes read this one number, so they cannot drift.
pub const RUN_GAP_MS: u64 = 300_000;

/// Whether an op at `now_ms` starts a run, given when its actor was last seen (`None`: never). It does when the actor
/// is new or was last seen more than `gap_ms` ago; a clock that goes back is the same run.
pub fn starts_run(last: Option<u64>, now_ms: u64, gap_ms: u64) -> bool {
    match last {
        Some(at) => now_ms.saturating_sub(at) > gap_ms,
        None => true,
    }
}

/// Tells the first op of a run from the rest.
#[derive(Debug, Clone)]
pub struct RunDetector {
    gap_ms: u64,
    last: HashMap<String, u64>,
}

impl RunDetector {
    /// A run ends when its actor is quiet for more than `gap_ms`.
    pub fn new(gap_ms: u64) -> Self {
        RunDetector {
            gap_ms,
            last: HashMap::new(),
        }
    }

    /// An op of `actor` at `now_ms`. True when it starts a run: the actor is new, or its last op was more than
    /// the gap ago. The clock is the caller's; a value that goes back counts as the same run.
    pub fn observe(&mut self, actor: &str, now_ms: u64) -> bool {
        let starts = starts_run(self.last.get(actor).copied(), now_ms, self.gap_ms);
        self.last.insert(actor.to_string(), now_ms);
        starts
    }
}
