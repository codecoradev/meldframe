//! When the server takes a checkpoint on its own: the start of an agent's run (ADR-0023).

use meldframe_revert::{RUN_GAP_MS, starts_run};
use std::collections::{BTreeMap, HashMap};

/// The most entries a [`RunWatch`] holds: past it the oldest entry goes.
pub const MAX_DETECTOR_ENTRIES: usize = 10_000;

/// Whether an actor string is an agent's: the `agent:` prefix, the way the lanes read it for display (the one
/// definition is `meldframe_text::is_agent`). Until the principal has a kind (ADR-0022) this is a guess that decides
/// nothing about access: a wrong one costs an extra automatic row or a missing one.
pub fn is_agent(actor: &str) -> bool {
    meldframe_text::is_agent(actor)
}

/// The `seq` of a run start: the file as it was just before the first op the run wrote, given the `seq` of every
/// op the call wrote. `None` when it wrote none: there is no run to start.
pub fn seq_before_first(written: impl IntoIterator<Item = u64>) -> Option<u64> {
    written
        .into_iter()
        .min()
        .map(|first| first.saturating_sub(1))
}

type Key = (String, String, String);

/// Tells the first write of an agent's run in a file from the rest, for the server. In memory and bounded: it holds
/// an entry only for an actor seen within the gap (an older entry and none give the same answer, so one older than the
/// gap is pruned on every call), and at the cap the oldest entry goes. After a restart it is empty, so the first
/// write of an agent counts as a run start. The comparison is the one `RunDetector` of the app layer uses
/// ([`starts_run`]) and the clock is the caller's. A clock that goes back is the same run, and the time kept for an
/// actor is the larger of the one stored and the one given (unlike `RunDetector`, which keeps the last), so a clock that
/// steps back and returns cannot make a later call look like a gap.
#[derive(Debug, Clone)]
pub struct RunWatch {
    gap_ms: u64,
    cap: usize,
    ticks: u64,
    /// Where each key was last seen, and the tick that tells entries of one instant apart.
    last: HashMap<Key, (u64, u64)>,
    by_age: BTreeMap<(u64, u64), Key>,
}

impl Default for RunWatch {
    fn default() -> Self {
        RunWatch::new()
    }
}

impl RunWatch {
    /// The shared gap ([`RUN_GAP_MS`]) and [`MAX_DETECTOR_ENTRIES`].
    pub fn new() -> Self {
        RunWatch::with_limits(RUN_GAP_MS, MAX_DETECTOR_ENTRIES)
    }

    /// A watch with its own gap and cap (a test's).
    pub fn with_limits(gap_ms: u64, cap: usize) -> Self {
        RunWatch {
            gap_ms,
            cap: cap.max(1),
            ticks: 0,
            last: HashMap::new(),
            by_age: BTreeMap::new(),
        }
    }

    /// A write of `actor` in `file` of `workspace` at `now_ms`. True when it starts a run: the actor is an agent and
    /// is new here, or its last write was more than the gap ago. A call of a non-agent does nothing and leaves no
    /// entry.
    pub fn starts(&mut self, workspace: &str, file: &str, actor: &str, now_ms: u64) -> bool {
        if !is_agent(actor) {
            return false;
        }
        self.prune(now_ms);
        let key = (workspace.to_owned(), file.to_owned(), actor.to_owned());
        let seen = self.last.remove(&key);
        match seen {
            Some(at) => {
                self.by_age.remove(&at);
            }
            None if self.last.len() >= self.cap => {
                if let Some((_, oldest)) = self.by_age.pop_first() {
                    self.last.remove(&oldest);
                }
            }
            None => {}
        }
        self.ticks += 1;
        // The larger of the two: a clock that steps back and then returns must not make a later call look like a gap.
        let stored = seen.map_or(now_ms, |(at, _)| at.max(now_ms));
        let at = (stored, self.ticks);
        self.by_age.insert(at, key.clone());
        self.last.insert(key, at);
        starts_run(seen.map(|(at, _)| at), now_ms, self.gap_ms)
    }

    /// Give a claim back: the entry of an actor is dropped, so its next write counts as a run start again. For a run
    /// start that was claimed and then not taken (the store failed, the job did not run): the entry it replaced was
    /// older than the gap or none, so dropping it is the state before the claim.
    pub fn forget(&mut self, workspace: &str, file: &str, actor: &str) {
        let key = (workspace.to_owned(), file.to_owned(), actor.to_owned());
        if let Some(at) = self.last.remove(&key) {
            self.by_age.remove(&at);
        }
    }

    /// How many entries it holds.
    pub fn len(&self) -> usize {
        self.last.len()
    }

    pub fn is_empty(&self) -> bool {
        self.last.is_empty()
    }

    fn prune(&mut self, now_ms: u64) {
        while let Some(entry) = self.by_age.first_entry() {
            if now_ms.saturating_sub(entry.key().0) <= self.gap_ms {
                break;
            }
            let key = entry.remove();
            self.last.remove(&key);
        }
    }
}

/// What a write tells the [`RunSink`] about a call: the file and the keys of every op the call carried (`{key}:{i}`),
/// so that a call that failed after it wrote a prefix is found by key.
#[derive(Debug)]
pub struct RunCall {
    pub file: String,
    pub keys: Vec<String>,
}

/// Told by the one place every write passes, after a call of an agent ran, to take a run start. The server sets it
/// once. It never changes the reply of the call, and it is two steps so the common path stays cheap.
pub trait RunSink: std::fmt::Debug + Send + Sync {
    /// In memory, never blocks: whether a write of `actor` in `file` of `workspace` now starts a run. Called after a
    /// call that wrote at least one op and, when it is true, followed by [`RunSink::take`].
    fn starts(&self, workspace: &str, file: &str, actor: &str) -> bool;

    /// Blocks, so it runs on the blocking pool: take the row of a run that started. `done` is the `(seq, written)` of
    /// each op of a call that succeeded, `written` false for a replay of one already in the log. `None` is a call that
    /// failed: the sink finds out by key how much of it was written, and only when some was does it ask
    /// [`RunSink::starts`], so a call that wrote nothing claims no run.
    fn take(&self, workspace: &str, actor: &str, call: &RunCall, done: Option<&[(u64, bool)]>);

    /// The job of [`RunSink::take`] did not run to its end (no slot was free, it outlasted the call's time limit, it
    /// panicked); `why` is the code of the failure. The sink logs and counts it and gives the claim back, so the next
    /// call of the run can take the run start. The job may still finish later, which costs one extra row.
    fn lost(&self, workspace: &str, file: &str, actor: &str, why: &str);
}
