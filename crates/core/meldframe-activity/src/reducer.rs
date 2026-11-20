//! The reducer of the activity panel (E5.1, ADR-0021): what the app layer folds every op, header, touch and tick into,
//! and what it reads back as lanes and rows. It is pure: time comes in as a number, nothing is read or written.
//!
//! Three things arrive with no order between them, and the rows must not depend on it: the ops of a batch (durable
//! frames, in log order), its header (an ephemeral frame, or a page of a fetch) and the clock. A header of the
//! highest `rev` per `n` wins, the count it carried goes with it, and a batch is judged by its ops and its header
//! together, the log winning where the cache of the header is lower.

use crate::summary::Tally;
use crate::{Fetch, Header, RawHeader};
use meldframe_revert::{LoggedOp, batch_index};
use std::collections::{BTreeMap, HashMap};

/// Lanes the state holds; the one with the oldest activity goes first.
pub const MAX_LANES: usize = 32;
/// Rows the state holds over all lanes; the oldest (lowest `firstSeq`) goes first.
pub const MAX_ROWS: usize = 500;
/// The highest `rev` a header may carry: one counter step per header write, so 2^48 is out of reach and a header
/// above it is a bad frame (it would otherwise freeze the count and turn every later header into a gap).
pub const MAX_REV: u64 = 1 << 48;
/// A frame whose `rev` is more than this above the highest held is not taken in (a lost run that long is not a lost
/// frame): it asks for the newest page instead. A page is the truth and has only [`MAX_REV`] to keep.
pub const MAX_REV_JUMP: u64 = 1_000_000;

/// A run ends when its actor is quiet for more than this (the `RunDetector` of the checkpoints uses it too).
pub const RUN_GAP_MS: u64 = 300_000;
/// A lane is `working` while its ops or touches arrived less than this ago.
pub const WORKING_MS: u64 = 10_000;
/// A lane is `idle` once this long has passed since its last op or touch.
pub const IDLE_MS: u64 = 60_000;
/// Heal rule (b): a prefix seen in ops with no header this long makes a fetch.
pub const WAIT_FOR_HEADER_MS: u64 = 3_000;
/// Heal rule (b): at most one fetch in this time, and this many tries for one prefix.
pub const FETCH_EVERY_MS: u64 = 5_000;
pub const MAX_TRIES: u8 = 3;
/// The page a fetch asks for.
pub const FETCH_LIMIT: u32 = 100;
/// A touch ring is gone this long after it came; at most this many are kept, of at most this many ids.
pub const RING_MS: u64 = 1_500;
pub const MAX_RINGS: usize = 50;
pub const MAX_RING_IDS: usize = 64;

/// A batch's identity: its actor and key prefix.
pub(crate) type Key = (String, String);

/// A ring the canvas draws for a moment: the nodes an actor's op touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ring {
    pub actor: String,
    pub ids: Vec<String>,
    pub at_ms: u64,
}

/// Everything known of one batch: the ops seen, the header held, and where the heal stands.
#[derive(Debug, Clone, Default)]
pub(crate) struct Group {
    pub header: Option<Header>,
    pub ops: u32,
    /// The lowest and highest `seq` among the ops seen.
    pub seqs: Option<(u64, u64)>,
    /// When the first op arrived, on the front end's clock.
    pub first_at: Option<u64>,
    pub tally: Tally,
    /// Fetches this prefix has been part of while it had no header.
    pub tries: u8,
    /// Headerless for good (until a header arrives): the tries are spent or the lane went idle.
    pub dead: bool,
}

/// The reducer. `new` takes the person's own actor, whose touches draw no ring.
///
/// Every `now_ms` is epoch milliseconds on the front end's clock, the same epoch as the server's `atMs` of a header
/// (a lane's last activity and its runs mix the two). A front end whose clock is ahead of or behind the server's by a
/// skew shows a lane's last activity that far in the future or the past, and a lane may sort a little off; nothing
/// is dropped for it.
#[derive(Debug, Clone)]
pub struct Activity {
    me: String,
    pub(crate) groups: BTreeMap<Key, Group>,
    by_n: HashMap<u32, Key>,
    /// Runs of ops that belong to no batch (a person's edits), per actor, oldest first.
    pub(crate) loose: HashMap<String, Vec<crate::Run>>,
    /// When each actor's last op or touch arrived, on the front end's clock.
    pub(crate) seen: HashMap<String, u64>,
    pub(crate) now: u64,
    last_seq: Option<u64>,
    highest_rev: u64,
    /// The server's `unreviewedCount` and the `rev` it was read at.
    count: (u64, u32),
    last_fetch: Option<u64>,
    /// A gap in `rev` was seen and not asked for (inside the window of [`FETCH_EVERY_MS`]); `tick` asks once it is due.
    gap_pending: bool,
    rings: Vec<Ring>,
}

impl Activity {
    pub fn new(me: &str) -> Self {
        Activity {
            me: me.to_owned(),
            groups: BTreeMap::new(),
            by_n: HashMap::new(),
            loose: HashMap::new(),
            seen: HashMap::new(),
            now: 0,
            last_seq: None,
            highest_rev: 0,
            count: (0, 0),
            last_fetch: None,
            gap_pending: false,
            rings: Vec::new(),
        }
    }

    /// The person's own actor, whose touches draw no ring.
    pub fn me(&self) -> &str {
        &self.me
    }

    /// Everything starts again, as for a new `Activity`, with the log at `base_seq`: headers, numbers, the count, the
    /// highest `rev` and the lanes' activity go. Use it when the server's numbering itself starts again (a restart of
    /// the in-memory server numbers `n` and `rev` from 1), and then fetch a page; [`reset_log`](Self::reset_log) would
    /// keep the old numbering and refuse every new header as stale.
    pub fn reset_all(&mut self, base_seq: u64) {
        *self = Activity::new(&self.me);
        self.last_seq = Some(base_seq);
    }

    /// The log starts again at `base_seq` with the same header numbering. The app layer does not call it: its replica
    /// turns the replay after a `Subscribed` or a `Resync` into duplicates before an op reaches the reducer, and the
    /// reducer ignores an op at or below the highest `seq` seen, so nothing is counted twice, and this call would drop
    /// the summary and Show of every row at each reconnect. A reload calls [`reset_all`](Self::reset_all) (through
    /// `Feed::reset`); a server restart that renumbers `n` and `rev` reaches the app layer only as that reload.
    ///
    /// Ops at or below `base_seq` are the snapshot's and count nothing; every op seen so far is forgotten (a replay from `base_seq` would count it twice), so a batch with no
    /// header goes and a batch with a header stays as a header-only row, with no summary and no Show, as after a
    /// reload. Headers, the count and when each lane was last active are kept.
    pub fn reset_log(&mut self, base_seq: u64) {
        self.last_seq = Some(base_seq);
        self.loose.clear();
        self.groups.retain(|_, g| g.header.is_some());
        for g in self.groups.values_mut() {
            *g = Group {
                header: g.header.take(),
                ..Group::default()
            };
        }
    }

    fn advance(&mut self, now_ms: u64) {
        self.now = self.now.max(now_ms);
        let now = self.now;
        self.rings.retain(|r| now.saturating_sub(r.at_ms) < RING_MS);
    }

    fn see(&mut self, actor: &str, now_ms: u64) {
        let at = self.seen.entry(actor.to_owned()).or_insert(0);
        *at = (*at).max(now_ms);
    }

    /// A confirmed op arrived at `now_ms`. Ops come in log order: one at or below the highest `seq` seen is a replay
    /// (a reconnect) and counts nothing. An op with a batch key joins its batch; any other is the actor's own edit
    /// and only feeds its runs.
    pub fn observe_op(&mut self, op: &LoggedOp, now_ms: u64) {
        self.advance(now_ms);
        if self.last_seq.is_some_and(|s| op.seq <= s) {
            return;
        }
        self.last_seq = Some(op.seq);
        self.see(&op.actor, now_ms);
        if let Some((prefix, _)) = batch_index(&op.key) {
            let g = self
                .groups
                .entry((op.actor.clone(), prefix.to_owned()))
                .or_default();
            g.ops = g.ops.saturating_add(1);
            let (lo, hi) = g.seqs.unwrap_or((op.seq, op.seq));
            g.seqs = Some((lo.min(op.seq), hi.max(op.seq)));
            g.first_at = Some(g.first_at.map_or(now_ms, |t| t.min(now_ms)));
            g.tally.add(&op.command);
        } else {
            let runs = self.loose.entry(op.actor.clone()).or_default();
            runs.push(crate::Run {
                start_ms: now_ms,
                end_ms: now_ms,
                ops: 1,
            });
            crate::view::merge_into(runs);
        }
        self.prune();
    }

    /// A touch hint arrived: it keeps its actor's lane `working`, and draws a ring unless the actor is the person.
    pub fn observe_touch(&mut self, actor: &str, ids: &[String], now_ms: u64) {
        self.advance(now_ms);
        self.see(actor, now_ms);
        if actor != self.me && !ids.is_empty() {
            let ids = ids.iter().take(MAX_RING_IDS).cloned().collect();
            self.rings.push(Ring {
                actor: actor.to_owned(),
                ids,
                at_ms: now_ms,
            });
            let extra = self.rings.len().saturating_sub(MAX_RINGS);
            self.rings.drain(..extra);
        }
        self.prune();
    }

    /// Whether a fetch may go out now (at most one in [`FETCH_EVERY_MS`]); if so, the time is taken.
    fn fetch_due(&mut self) -> bool {
        let due = self
            .last_fetch
            .is_none_or(|t| self.now.saturating_sub(t) >= FETCH_EVERY_MS);
        if due {
            self.last_fetch = Some(self.now);
            self.gap_pending = false;
        }
        due
    }

    /// A `Batch` frame: one header and the `unreviewedCount` the server sent with it. Returns the fetch of heal rule
    /// (a) when its `rev` is more than 1 above the highest held (a frame was lost); it is asked for once, because the
    /// highest `rev` then moves up to this one. A frame more than [`MAX_REV_JUMP`] above the highest held (from 0 for
    /// a fresh reducer too) is not taken in; it asks for the newest page, at most once in [`FETCH_EVERY_MS`] (inside
    /// the window it returns `None`), and the page ([`observe_page`](Self::observe_page)) brings the numbers in.
    pub fn observe_header(
        &mut self,
        header: impl Into<RawHeader>,
        unreviewed: u32,
    ) -> Option<Fetch> {
        let h = Into::<RawHeader>::into(header).check()?;
        let fetch = Fetch {
            before_n: None,
            after_n: None,
            limit: FETCH_LIMIT,
        };
        if h.rev > self.highest_rev.saturating_add(MAX_REV_JUMP) {
            return self.fetch_due().then_some(fetch);
        }
        let gap = h.rev > self.highest_rev.saturating_add(1);
        self.highest_rev = self.highest_rev.max(h.rev);
        if h.rev >= self.count.0 {
            self.count = (h.rev, unreviewed);
        }
        self.take(h);
        self.prune();
        let asked = gap && self.fetch_due();
        self.gap_pending |= gap && !asked;
        asked.then_some(fetch)
    }

    /// The answer of a fetch: a page of headers and the `unreviewedCount` read with it. The count of the page wins only
    /// when the highest `rev` of the page is above the `rev` of the count held, so a frame as new as the page, or newer,
    /// keeps its own.
    pub fn observe_page(&mut self, headers: impl IntoIterator<Item = RawHeader>, unreviewed: u32) {
        let mut top = None;
        for h in headers.into_iter().filter_map(RawHeader::check) {
            top = top.max(Some(h.rev));
            self.highest_rev = self.highest_rev.max(h.rev);
            self.take(h);
        }
        if let Some(rev) = top.filter(|r| *r > self.count.0) {
            self.count = (rev, unreviewed);
        }
        self.prune();
    }

    /// One row of the review route's answer: the row after a person's change. It takes the row in like a page does,
    /// and touches neither the count (the answer carries none) nor the highest `rev` (a frame lost before it is still
    /// a gap).
    pub fn observe_row(&mut self, header: impl Into<RawHeader>) {
        if let Some(h) = Into::<RawHeader>::into(header).check() {
            self.take(h);
            self.prune();
        }
    }

    /// Put a checked header on its batch if it is newer than the one held for its `n`. A header that names another
    /// batch than the one held for that `n` moves; a batch that already holds a header of another `n` keeps the one of
    /// the higher `rev` (the server never does either, so this only keeps the state consistent).
    fn take(&mut self, h: Header) {
        let key = (h.actor.clone(), h.key_prefix.clone());
        let held = |s: &Self, k: &Key| {
            s.groups
                .get(k)
                .and_then(|g| g.header.as_ref())
                .map(|h| (h.n, h.rev))
        };
        // Both refusals come before anything moves, so a refused header changes nothing.
        let old = self.by_n.get(&h.n).cloned();
        if old
            .as_ref()
            .and_then(|k| held(self, k))
            .is_some_and(|(_, rev)| rev >= h.rev)
        {
            return;
        }
        let other = held(self, &key).filter(|(n, _)| *n != h.n);
        if other.is_some_and(|(_, rev)| rev >= h.rev) {
            return;
        }
        if let Some(old) = old.filter(|o| *o != key) {
            self.release(&old);
        }
        if let Some((n, _)) = other {
            self.by_n.remove(&n);
        }
        self.by_n.insert(h.n, key.clone());
        self.groups.entry(key).or_default().header = Some(h);
    }

    /// Take the header off a batch; a batch with no op seen goes with it.
    fn release(&mut self, key: &Key) {
        if let Some(g) = self.groups.get_mut(key) {
            g.header = None;
            if g.ops == 0 {
                self.groups.remove(key);
            }
        }
    }

    /// The clock moved to `now_ms` and nothing can be asked (the connection is down): batches of a lane that went idle
    /// are headerless for good, and no try is spent.
    pub fn clock(&mut self, now_ms: u64) {
        self.advance(now_ms);
        let (now, seen) = (self.now, &self.seen);
        for ((actor, _), g) in self.groups.iter_mut().filter(|(_, g)| g.header.is_none()) {
            if seen
                .get(actor)
                .is_some_and(|t| now.saturating_sub(*t) >= IDLE_MS)
            {
                g.dead = true;
            }
        }
    }

    /// The clock moved to `now_ms`. Marks the batches that are headerless for good (the lane went idle) and applies
    /// heal rule (b): a batch seen in ops for 3 seconds with no header (and fewer than 3 tries) is asked for, in one
    /// fetch of what is newer than the highest `n` held, at most one in 5 seconds; every batch waiting then spends a
    /// try, and the third makes it final.
    pub fn tick(&mut self, now_ms: u64) -> Option<Fetch> {
        self.advance(now_ms);
        let now = self.now;
        let due = self
            .last_fetch
            .is_none_or(|t| now.saturating_sub(t) >= FETCH_EVERY_MS);
        let mut asked = false;
        let seen = &self.seen;
        for ((actor, _), g) in self
            .groups
            .iter_mut()
            .filter(|(_, g)| g.header.is_none() && !g.dead)
        {
            if seen
                .get(actor)
                .is_some_and(|t| now.saturating_sub(*t) >= IDLE_MS)
            {
                g.dead = true;
            } else if due
                && g.first_at
                    .is_some_and(|t| now.saturating_sub(t) >= WAIT_FOR_HEADER_MS)
            {
                g.tries += 1;
                g.dead = g.tries >= MAX_TRIES;
                asked = true;
            }
        }
        // A gap seen inside the window is asked for now, as the newest page: the hole may be a change of an older row.
        let gap = due && std::mem::take(&mut self.gap_pending);
        (asked || gap).then(|| {
            self.last_fetch = Some(now);
            Fetch {
                before_n: None,
                after_n: if gap { None } else { self.highest_n() },
                limit: FETCH_LIMIT,
            }
        })
    }

    /// A fetch goes out now, asked for by the caller (rule (d) after a subscribe): the window of [`FETCH_EVERY_MS`]
    /// opens, so rules (a) and (b) do not ask for the same headers again at once, and a gap waiting for it is healed.
    pub fn spend_window(&mut self, now_ms: u64) {
        self.advance(now_ms);
        self.last_fetch = Some(self.now);
        self.gap_pending = false;
    }

    fn drop_group(&mut self, key: &Key) {
        self.groups.remove(key);
        self.by_n.retain(|_, k| k != key);
    }

    /// Hold the bounds: 500 rows (the lowest `firstSeq` goes), then 32 lanes (the oldest activity goes, rows and all).
    fn prune(&mut self) {
        while self.groups.len() > MAX_ROWS {
            let oldest = self
                .groups
                .iter()
                .min_by_key(|(k, g)| (g.span().0, *k))
                .map(|(k, _)| k.clone());
            oldest.iter().for_each(|k| self.drop_group(k));
        }
        loop {
            let last = self.last_at();
            if last.len() <= MAX_LANES {
                return;
            }
            let Some(gone) = last
                .into_iter()
                .min_by(|a, b| (a.1, &a.0).cmp(&(b.1, &b.0)))
                .map(|(a, _)| a)
            else {
                return;
            };
            let keys: Vec<Key> = self
                .groups
                .keys()
                .filter(|k| k.0 == gone)
                .cloned()
                .collect();
            keys.iter().for_each(|k| self.drop_group(k));
            self.loose.remove(&gone);
            self.seen.remove(&gone);
        }
    }

    /// The `unreviewedCount` of the highest `rev` seen: the server's number, never recounted from the rows held.
    pub fn unreviewed_count(&self) -> u32 {
        self.count.1
    }

    pub fn highest_rev(&self) -> u64 {
        self.highest_rev
    }

    pub fn highest_n(&self) -> Option<u32> {
        self.by_n.keys().max().copied()
    }

    pub fn lowest_n(&self) -> Option<u32> {
        self.by_n.keys().min().copied()
    }

    /// The rings the canvas draws now, oldest first.
    pub fn rings(&self) -> &[Ring] {
        &self.rings
    }

    /// The node ids a batch's Show selects: the nodes its ops named and did not remove. None for a batch the ops of
    /// which were not seen (a header-only row has no Show).
    pub fn show_ids(&self, actor: &str, key_prefix: &str) -> Vec<String> {
        let key = (actor.to_owned(), key_prefix.to_owned());
        self.groups
            .get(&key)
            .map(|g| g.tally.shown.clone())
            .unwrap_or_default()
    }
}
