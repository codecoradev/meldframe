//! When a checkpoint is due on its own (E5.3): at the start of an agent run.
//!
//! The log carries no time, so the caller gives each op's clock value (the app layer gets it as the payload of
//! the action that delivers the op). A run is a stretch of ops by one actor with no pause longer than the gap.

use std::collections::HashMap;

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
        let starts = match self.last.get(actor) {
            Some(&at) => now_ms.saturating_sub(at) > self.gap_ms,
            None => true,
        };
        self.last.insert(actor.to_string(), now_ms);
        starts
    }
}
