//! What the reducer shows: the rows, derived from the groups and the headers.

use crate::reducer::{Group, Key};
use crate::summary::Summary;
use crate::{Activity, Note, Review};

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
}
