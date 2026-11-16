//! Touch hints: who touched which nodes of a file, for the editor's touch ring. A hint is ephemeral (never
//! stored, never replayed) and carries only ids. [`Touches`] decides when one may go out: at most one per
//! actor and file per window, and the latest node ids win. It is pure: the caller passes `now`.

use std::collections::{HashMap, HashSet};

/// The most node ids one hint names; the rest of a very large batch is left out.
pub const MAX_TOUCH_NODES: usize = 64;

/// The most `(file, actor)` pairs tracked at once. A new pair arriving at the limit, with every tracked pair
/// still inside its window, gets no hint (they are best effort); a pair past its window is forgotten.
pub const MAX_TOUCH_ENTRIES: usize = 1024;

/// A hint to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchHint {
    pub actor: String,
    pub node_ids: Vec<String>,
    /// The sequence number of the newest op the hint is about.
    pub seq: u64,
}

#[derive(Debug)]
struct Entry {
    /// When a hint was last sent for this pair, `None` before the first.
    last_sent: Option<u64>,
    /// A hint that arrived inside the window and waits for it to end; a newer one replaces it.
    pending: Option<TouchHint>,
}

/// The coalescer: one entry per `(file key, actor)`.
#[derive(Debug)]
pub struct Touches {
    window_ms: u64,
    entries: HashMap<(String, String), Entry>,
    waiting: usize,
}

impl Touches {
    /// Send at most one hint per actor and file every `window_ms` (0 sends every hint).
    pub fn new(window_ms: u64) -> Self {
        Touches {
            window_ms,
            entries: HashMap::new(),
            waiting: 0,
        }
    }

    /// Pairs tracked now (at most [`MAX_TOUCH_ENTRIES`]).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// An actor touched `node_ids` of file `key`, the newest op being `seq`. Returns the hint to send now:
    /// the first one, or one after the window. Inside the window the hint waits (it replaces a waiting one)
    /// and [`Touches::due`] returns it when the window is over. A hint with no node ids is not sent.
    pub fn record(
        &mut self,
        key: &str,
        actor: &str,
        node_ids: Vec<String>,
        seq: u64,
        now: u64,
    ) -> Option<TouchHint> {
        let node_ids = unique(node_ids);
        if node_ids.is_empty() {
            return None;
        }
        let hint = TouchHint {
            actor: actor.to_owned(),
            node_ids,
            seq,
        };
        let pair = (key.to_owned(), actor.to_owned());
        if !self.entries.contains_key(&pair) && self.entries.len() >= MAX_TOUCH_ENTRIES {
            self.forget_idle(now);
            if self.entries.len() >= MAX_TOUCH_ENTRIES {
                return None;
            }
        }
        let window = self.window_ms;
        let entry = self.entries.entry(pair).or_insert(Entry {
            last_sent: None,
            pending: None,
        });
        if entry.last_sent.is_none_or(|sent| over(now, sent, window)) {
            // A newer hint supersedes one that was still waiting.
            self.waiting -= usize::from(entry.pending.take().is_some());
            entry.last_sent = Some(now);
            return Some(hint);
        }
        self.waiting += usize::from(entry.pending.replace(hint).is_none());
        None
    }

    /// The hints whose window is over at `now`, as `(file key, hint)`. Each is returned once.
    pub fn due(&mut self, now: u64) -> Vec<(String, TouchHint)> {
        if self.waiting == 0 {
            return Vec::new();
        }
        let window = self.window_ms;
        let mut out = Vec::new();
        for ((key, _), entry) in &mut self.entries {
            let ended = entry.last_sent.is_none_or(|sent| over(now, sent, window));
            if ended && entry.pending.is_some() {
                entry.last_sent = Some(now);
                out.extend(entry.pending.take().map(|h| (key.clone(), h)));
            }
        }
        self.waiting -= out.len();
        out
    }

    /// Drop the pairs with nothing waiting whose window is over: sending for them again starts a new window.
    fn forget_idle(&mut self, now: u64) {
        let window = self.window_ms;
        self.entries.retain(|_, e| {
            e.pending.is_some() || e.last_sent.is_some_and(|s| !over(now, s, window))
        });
    }
}

/// Whether the window that started at `sent` is over at `now`. A clock that stepped back (`now < sent`)
/// counts as over, so hints are never held for a window that lies in the future.
fn over(now: u64, sent: u64, window: u64) -> bool {
    now < sent || now - sent >= window
}

/// The ids once each, in order, at most [`MAX_TOUCH_NODES`].
fn unique(ids: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    ids.into_iter()
        .filter(|id| seen.insert(id.clone()))
        .take(MAX_TOUCH_NODES)
        .collect()
}
