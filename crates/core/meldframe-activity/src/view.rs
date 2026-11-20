//! What the reducer shows: rows, lanes and runs, all derived from the groups, the clock and the headers.

use crate::reducer::{Group, IDLE_MS, Key, RUN_GAP_MS, WORKING_MS};
use crate::summary::Summary;
use crate::{Activity, Note, Review};
use std::collections::HashMap;

/// The runs a lane lists (newest first), and the runs of loose ops kept per actor.
pub const MAX_RUNS: usize = 20;
const MAX_LOOSE_RUNS: usize = 64;

/// How far a batch got: `building` (ops seen, no header yet, the heal still trying), `applied` (`written` reached
/// `commands`), `partial` (fewer written: "18 of 30"), `headerless` (ops seen, no header, and the heal gave up or the
/// lane went idle; final until a header arrives). A refused call is not a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Building,
    Applied,
    Partial,
    Headerless,
}

/// One batch. A row with no header has no number, no note, no review (it reads `unreviewed`, for display only),
/// `commands` as `written` and `rev` 0; a row with a header and no ops seen has no summary and no Show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub actor: String,
    pub key_prefix: String,
    pub n: Option<u32>,
    pub rev: u64,
    pub first_seq: u64,
    pub last_seq: u64,
    pub commands: u32,
    pub written: u32,
    pub atomic: bool,
    pub at_ms: u64,
    pub note: Option<Note>,
    pub status: Status,
    pub review: Review,
    pub reviewed_by: Option<String>,
    pub reviewed_at_ms: Option<u64>,
    pub summary: Option<Summary>,
    pub can_show: bool,
}

/// A stretch of one actor's work between two gaps of [`RUN_GAP_MS`]: when it began and ended and how many ops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub start_ms: u64,
    pub end_ms: u64,
    pub ops: u32,
}

/// `agent:` is an agent, anything else a person: for display only, it decides nothing about who may review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneKind {
    Agent,
    Person,
}

/// `working` for 10 seconds after an op or a touch, `idle` from 60 seconds, `quiet` in between (the ADR names the
/// two ends only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneState {
    Working,
    Quiet,
    Idle,
}

/// One actor in one file. `batches` are its key prefixes, newest first; the rows are those of [`Activity::rows`].
///
/// `ops` is the number of ops the state holds for the lane now, not a running total: it can fall when a batch row or
/// a run is dropped at a bound, when the log is reset, or when a header's `written` replaces a smaller count of ops
/// seen. `last_at_ms` is the latest of the lane's arrivals (front end clock) and its headers' `atMs` (server clock),
/// both epoch milliseconds: with a skew between the two clocks it can sit a little in the future.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lane {
    pub actor: String,
    pub kind: LaneKind,
    pub state: LaneState,
    pub ops: u32,
    pub last_at_ms: u64,
    pub runs: Vec<Run>,
    pub batches: Vec<String>,
}

/// Merge runs that are no more than [`RUN_GAP_MS`] apart; the result is oldest first.
fn merge(mut v: Vec<Run>) -> Vec<Run> {
    v.sort_by_key(|r| (r.start_ms, r.end_ms, r.ops));
    let mut out: Vec<Run> = Vec::with_capacity(v.len());
    for r in v {
        match out.last_mut() {
            Some(last) if r.start_ms.saturating_sub(last.end_ms) <= RUN_GAP_MS => {
                last.end_ms = last.end_ms.max(r.end_ms);
                last.ops = last.ops.saturating_add(r.ops);
            }
            _ => out.push(r),
        }
    }
    out
}

/// Merge in place, keeping the newest [`MAX_LOOSE_RUNS`].
pub(crate) fn merge_into(runs: &mut Vec<Run>) {
    *runs = merge(std::mem::take(runs));
    let extra = runs.len().saturating_sub(MAX_LOOSE_RUNS);
    runs.drain(..extra);
}

impl Group {
    /// The lowest and highest `seq` of the batch, from the ops seen and the header together (the log wins where it
    /// holds more; the header still knows the ops that were never seen).
    pub(crate) fn span(&self) -> (u64, u64) {
        let h = self.header.as_ref().map(|h| (h.first_seq, h.last_seq));
        match (self.seqs, h) {
            (Some((a, b)), Some((c, d))) => (a.min(c), b.max(d)),
            (Some(s), None) | (None, Some(s)) => s,
            (None, None) => (0, 0),
        }
    }

    /// Ops known: the ops seen or what the header says was written, whichever is more.
    fn written(&self) -> u32 {
        self.ops.max(self.header.as_ref().map_or(0, |h| h.written))
    }

    /// The batch as one point of its lane's runs: the server's time when there is a header, else the arrival.
    fn point(&self) -> Run {
        let at = self
            .header
            .as_ref()
            .map(|h| h.at_ms)
            .or(self.first_at)
            .unwrap_or(0);
        Run {
            start_ms: at,
            end_ms: at,
            ops: self.written(),
        }
    }

    fn row(&self, (actor, key_prefix): &Key) -> Row {
        let (first_seq, last_seq) = self.span();
        let written = self.written();
        let h = self.header.as_ref();
        let commands = h.map_or(written, |h| h.commands.max(written));
        let status = match h {
            Some(_) if written >= commands => Status::Applied,
            Some(_) => Status::Partial,
            None if self.dead => Status::Headerless,
            None => Status::Building,
        };
        Row {
            actor: actor.clone(),
            key_prefix: key_prefix.clone(),
            n: h.map(|h| h.n),
            rev: h.map_or(0, |h| h.rev),
            first_seq,
            last_seq,
            commands,
            written,
            atomic: h.is_some_and(|h| h.atomic),
            at_ms: h.map(|h| h.at_ms).or(self.first_at).unwrap_or(0),
            note: h.and_then(|h| h.note.clone()),
            status,
            review: h.map_or(Review::Unreviewed, |h| h.review),
            reviewed_by: h.and_then(|h| h.reviewed_by.clone()),
            reviewed_at_ms: h.and_then(|h| h.reviewed_at_ms),
            summary: (self.ops > 0).then(|| self.tally.summary()),
            can_show: !self.tally.shown.is_empty(),
        }
    }
}

impl Activity {
    /// The rows, newest first (by `firstSeq`, as the ADR orders them; the number `n` is a label, not a position).
    pub fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = self.groups.iter().map(|(k, g)| g.row(k)).collect();
        rows.sort_by(|a, b| {
            (b.first_seq, &b.actor, &b.key_prefix).cmp(&(a.first_seq, &a.actor, &a.key_prefix))
        });
        rows
    }

    /// When each actor was last active: the latest of its ops and touches (front end clock) and its headers' `atMs`.
    pub(crate) fn last_at(&self) -> HashMap<String, u64> {
        let mut last = self.seen.clone();
        let mut bump = |actor: &String, at: u64| {
            let e = last.entry(actor.clone()).or_insert(0);
            *e = (*e).max(at);
        };
        for ((actor, _), g) in &self.groups {
            bump(actor, g.header.as_ref().map_or(0, |h| h.at_ms));
        }
        for (actor, runs) in &self.loose {
            bump(actor, runs.last().map_or(0, |r| r.end_ms));
        }
        last
    }

    /// The lanes, the one with the newest activity first.
    pub fn lanes(&self) -> Vec<Lane> {
        let mut points: HashMap<&str, Vec<Run>> = HashMap::new();
        for ((actor, _), g) in &self.groups {
            points.entry(actor).or_default().push(g.point());
        }
        for (actor, runs) in &self.loose {
            points
                .entry(actor)
                .or_default()
                .extend(runs.iter().cloned());
        }
        let mut batches: HashMap<String, Vec<String>> = HashMap::new();
        for r in self.rows() {
            batches.entry(r.actor).or_default().push(r.key_prefix);
        }
        let mut lanes: Vec<Lane> = self
            .last_at()
            .into_iter()
            .map(|(actor, last_at_ms)| {
                let runs = merge(points.remove(actor.as_str()).unwrap_or_default());
                let age = self.seen.get(&actor).map(|t| self.now.saturating_sub(*t));
                Lane {
                    kind: if actor.starts_with("agent:") {
                        LaneKind::Agent
                    } else {
                        LaneKind::Person
                    },
                    state: match age {
                        Some(a) if a < WORKING_MS => LaneState::Working,
                        Some(a) if a < IDLE_MS => LaneState::Quiet,
                        _ => LaneState::Idle,
                    },
                    ops: runs.iter().fold(0, |n, r| n.saturating_add(r.ops)),
                    last_at_ms,
                    runs: runs.into_iter().rev().take(MAX_RUNS).collect(),
                    batches: batches.remove(&actor).unwrap_or_default(),
                    actor,
                }
            })
            .collect();
        lanes.sort_by(|a, b| (b.last_at_ms, &a.actor).cmp(&(a.last_at_ms, &b.actor)));
        lanes
    }
}
