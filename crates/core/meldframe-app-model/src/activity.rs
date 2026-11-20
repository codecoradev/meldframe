//! The activity panel as plain data (E5.1, ADR-0021): the state a front end draws and the batch header it hands
//! back. The rules are in `meldframe-activity` (the reducer and `Feed::view`) and `meldframe-app` (what feeds them);
//! this file only converts what `Feed::view` gives into serialisable data, and decides nothing.

use crate::action::Frame;
use meldframe_activity as act;
use meldframe_activity::{Feed, Fetch, InEffect, Page, ViewRing, ViewRow};
use meldframe_protocol::Node;
use meldframe_revert::ConfirmedLog;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A person's mark on a batch, spelled as the server spells it (`unreviewed`, `accepted`, `needs_review`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Unreviewed,
    Accepted,
    NeedsReview,
}

impl From<act::Review> for ReviewState {
    fn from(r: act::Review) -> Self {
        match r {
            act::Review::Unreviewed => Self::Unreviewed,
            act::Review::Accepted => Self::Accepted,
            act::Review::NeedsReview => Self::NeedsReview,
        }
    }
}

impl From<ReviewState> for act::Review {
    fn from(r: ReviewState) -> Self {
        match r {
            ReviewState::Unreviewed => Self::Unreviewed,
            ReviewState::Accepted => Self::Accepted,
            ReviewState::NeedsReview => Self::NeedsReview,
        }
    }
}

/// A batch header as the server sent it: a row of the list route or the body of a `batch` frame. Nothing in it is
/// trusted: the app layer checks it again (the note most of all) before a row is made from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchHeader {
    pub n: u32,
    #[ts(type = "number")]
    pub rev: u64,
    pub actor: String,
    pub key_prefix: String,
    pub tool: String,
    pub commands: u32,
    pub written: u32,
    pub atomic: bool,
    #[ts(type = "number")]
    pub first_seq: u64,
    #[ts(type = "number")]
    pub last_seq: u64,
    #[ts(type = "number")]
    pub at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub note: Option<String>,
    pub review: ReviewState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reviewed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub reviewed_at_ms: Option<u64>,
}

impl From<BatchHeader> for act::RawHeader {
    fn from(h: BatchHeader) -> Self {
        act::RawHeader {
            n: h.n,
            rev: h.rev,
            actor: h.actor,
            key_prefix: h.key_prefix,
            tool: h.tool,
            commands: h.commands,
            written: h.written,
            atomic: h.atomic,
            first_seq: h.first_seq,
            last_seq: h.last_seq,
            at_ms: h.at_ms,
            note: h.note,
            review: h.review.into(),
            reviewed_by: h.reviewed_by,
            reviewed_at_ms: h.reviewed_at_ms,
        }
    }
}

/// The answer of the list route (`GET /v1/files/{file}/batches`), as the route sends it: a page of headers newest
/// first, whether older ones remain below it, the server's count of headers that are not `accepted`, and whether
/// this person may set review states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchPage {
    pub batches: Vec<BatchHeader>,
    pub has_more: bool,
    pub unreviewed_count: u32,
    pub can_review: bool,
}

impl From<BatchPage> for Page {
    fn from(p: BatchPage) -> Self {
        Page {
            headers: p.batches.into_iter().map(Into::into).collect(),
            unreviewed: p.unreviewed_count,
            has_more: p.has_more,
            can_review: p.can_review,
        }
    }
}

impl Frame {
    /// Hand what this frame tells the feed over (a batch header or a touch hint): a plain mapping; the order the feed
    /// takes things in is its own (`Feed::header`). The fetch that heal rule (a) asks for, if any.
    pub fn observe(self, feed: &mut Feed, now_ms: u64) -> Option<Fetch> {
        match self {
            Frame::Batch {
                header,
                unreviewed_count,
            } => feed.header(now_ms, header, unreviewed_count),
            Frame::Touch { actor, ids, .. } => {
                feed.touch(now_ms, &actor, &ids);
                None
            }
            _ => None,
        }
    }
}

/// How far a batch got: `building` (ops seen, no header yet), `applied`, `partial` ("18 of 30") or `headerless`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum BatchStatus {
    Building,
    Applied,
    Partial,
    Headerless,
}

/// What a batch's ops did: counts, and at most three node names of at most 40 characters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchSummary {
    pub added: u32,
    pub changed: u32,
    pub removed: u32,
    pub names: Vec<String>,
}

/// One batch of one lane. A row with no header has no `n`, note or review (it reads `unreviewed` for display only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchRow {
    pub actor: String,
    pub key_prefix: String,
    pub n: Option<u32>,
    #[ts(type = "number")]
    pub rev: u64,
    #[ts(type = "number")]
    pub first_seq: u64,
    #[ts(type = "number")]
    pub last_seq: u64,
    pub commands: u32,
    pub written: u32,
    pub atomic: bool,
    #[ts(type = "number")]
    pub at_ms: u64,
    /// Plain text written by `actor`: show it quoted and attributed, never as markup.
    pub note: Option<String>,
    pub status: BatchStatus,
    pub review: ReviewState,
    pub reviewed_by: Option<String>,
    #[ts(type = "number | null")]
    pub reviewed_at_ms: Option<u64>,
    /// Counts of the ops the app has seen; `None` for a row made from a header alone.
    pub summary: Option<BatchSummary>,
    /// Whether Show has nodes to select.
    pub can_show: bool,
    /// Whether the app holds every op of the batch, so a revert can be worked out.
    pub revertable: bool,
    /// Worked out for the selected row only: whether a revert would still change anything (`None`: not known).
    pub in_effect: Option<bool>,
}

/// A batch's identity: its actor and key prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchRef {
    pub actor: String,
    pub key_prefix: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum LaneKind {
    Agent,
    Person,
}

/// `working` for 10 seconds after an op or touch, `quiet` up to 60 seconds, then `idle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum LaneState {
    Working,
    Quiet,
    Idle,
}

/// A stretch of one actor's work between two gaps of 5 minutes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityRun {
    #[ts(type = "number")]
    pub start_ms: u64,
    #[ts(type = "number")]
    pub end_ms: u64,
    pub ops: u32,
}

/// One actor in one file. `batches` are its key prefixes, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityLane {
    pub actor: String,
    pub kind: LaneKind,
    pub state: LaneState,
    pub ops: u32,
    #[ts(type = "number")]
    pub last_at_ms: u64,
    pub runs: Vec<ActivityRun>,
    pub batches: Vec<String>,
}

/// A touch hint: nodes an actor is changing. A ring that is not `persistent` is gone 1.5 seconds after it came; a
/// persistent one covers the top-level nodes of the newest batch of a working lane (at most 16 ids).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TouchRing {
    pub actor: String,
    pub ids: Vec<String>,
    #[ts(type = "number")]
    pub at_ms: u64,
    pub persistent: bool,
}

/// The Activity panel (E5.1): lanes (newest activity first, at most 32), batch rows (newest first, at most 500),
/// the selected row, the count of batches nobody accepted and the touch rings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityState {
    /// The server's count of headers that are not `accepted` (never recounted from the rows held).
    pub unreviewed_count: u32,
    /// Whether the server lets this person set a review state: show the controls only then.
    pub can_review: bool,
    pub lanes: Vec<ActivityLane>,
    pub rows: Vec<BatchRow>,
    pub selected: Option<BatchRef>,
    /// Whether `LoadOlderBatches` has older batches to fetch.
    pub has_older: bool,
    pub touches: Vec<TouchRing>,
}

impl From<ViewRow> for BatchRow {
    fn from(v: ViewRow) -> Self {
        let r = v.row;
        BatchRow {
            actor: r.actor,
            key_prefix: r.key_prefix,
            n: r.n,
            rev: r.rev,
            first_seq: r.first_seq,
            last_seq: r.last_seq,
            commands: r.commands,
            written: r.written,
            atomic: r.atomic,
            at_ms: r.at_ms,
            note: r.note.map(|n| n.to_string()),
            status: match r.status {
                act::Status::Building => BatchStatus::Building,
                act::Status::Applied => BatchStatus::Applied,
                act::Status::Partial => BatchStatus::Partial,
                act::Status::Headerless => BatchStatus::Headerless,
            },
            review: r.review.into(),
            reviewed_by: r.reviewed_by,
            reviewed_at_ms: r.reviewed_at_ms,
            summary: r.summary.map(|s| BatchSummary {
                added: s.added,
                changed: s.changed,
                removed: s.removed,
                names: s.names,
            }),
            can_show: r.can_show,
            revertable: v.revertable,
            in_effect: match v.in_effect {
                InEffect::Yes => Some(true),
                InEffect::No => Some(false),
                InEffect::Unknown => None,
            },
        }
    }
}

impl From<ViewRing> for TouchRing {
    fn from(r: ViewRing) -> Self {
        TouchRing {
            actor: r.actor,
            ids: r.ids,
            at_ms: r.at_ms,
            persistent: r.persistent,
        }
    }
}

impl ActivityLane {
    fn of(l: act::Lane) -> Self {
        ActivityLane {
            actor: l.actor,
            kind: match l.kind {
                act::LaneKind::Agent => LaneKind::Agent,
                act::LaneKind::Person => LaneKind::Person,
            },
            state: match l.state {
                act::LaneState::Working => LaneState::Working,
                act::LaneState::Quiet => LaneState::Quiet,
                act::LaneState::Idle => LaneState::Idle,
            },
            ops: l.ops,
            last_at_ms: l.last_at_ms,
            runs: l
                .runs
                .into_iter()
                .map(|r| ActivityRun {
                    start_ms: r.start_ms,
                    end_ms: r.end_ms,
                    ops: r.ops,
                })
                .collect(),
            batches: l.batches,
        }
    }
}

impl ActivityState {
    pub fn row(&self, b: &BatchRef) -> Option<&BatchRow> {
        self.rows
            .iter()
            .find(|r| r.actor == b.actor && r.key_prefix == b.key_prefix)
    }

    /// Bring the panel up to date with what `Feed::view` says; whether it changed.
    pub fn refresh(
        &mut self,
        feed: &mut Feed,
        log: &ConfirmedLog,
        seq: u64,
        nodes: &[Node],
    ) -> bool {
        let v = feed.view(log, seq, nodes);
        let next = ActivityState {
            unreviewed_count: v.unreviewed,
            can_review: v.can_review,
            has_older: v.has_older,
            lanes: v.lanes.into_iter().map(ActivityLane::of).collect(),
            rows: v.rows.into_iter().map(Into::into).collect(),
            selected: v
                .selected
                .map(|(actor, key_prefix)| BatchRef { actor, key_prefix }),
            touches: v.rings.into_iter().map(Into::into).collect(),
        };
        let changed = next != *self;
        *self = next;
        changed
    }
}
