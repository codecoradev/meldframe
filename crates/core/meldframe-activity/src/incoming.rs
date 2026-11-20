//! What the reducer takes in from the wire (a header as a socket or a route sent it) and what it asks for.

use crate::{
    Header, MAX_ACTOR, MAX_COUNT, MAX_KEY_PREFIX, MAX_REV, MAX_TOOL, Note, Review, is_hidden_text,
};

/// A fetch of headers (`FetchBatches` in the app layer): the page newest first, below `before_n`, above `after_n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fetch {
    pub before_n: Option<u32>,
    pub after_n: Option<u32>,
    pub limit: u32,
}

/// A header as the front end received it: nothing in it is trusted, the note least of all. [`RawHeader::check`] is
/// the app layer's second check; the reducer runs it on every header it takes in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawHeader {
    pub n: u32,
    pub rev: u64,
    pub actor: String,
    pub key_prefix: String,
    pub tool: String,
    pub commands: u32,
    pub written: u32,
    pub atomic: bool,
    pub first_seq: u64,
    pub last_seq: u64,
    pub at_ms: u64,
    pub note: Option<String>,
    pub review: Review,
    pub reviewed_by: Option<String>,
    pub reviewed_at_ms: Option<u64>,
}

impl From<Header> for RawHeader {
    fn from(h: Header) -> Self {
        RawHeader {
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
            note: h.note.map(|n| n.to_string()),
            review: h.review,
            reviewed_by: h.reviewed_by,
            reviewed_at_ms: h.reviewed_at_ms,
        }
    }
}

/// A name the rows may show: not empty, within `max` bytes, no hidden character.
fn plain(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && !s.chars().any(is_hidden_text)
}

impl RawHeader {
    /// The second check. `None` when the header cannot be a row: no number, a `rev` above [`MAX_REV`], or an actor, a key prefix or a tool that is
    /// empty, too long or holds a hidden character. Otherwise the counts are held to their bounds (`written` never
    /// above `commands`, `last_seq` never below `first_seq`), the note is cleaned as [`Note::clean`] does (cut,
    /// never refused), and `reviewed_by` and `reviewed_at_ms` stay only as a pair.
    pub fn check(self) -> Option<Header> {
        if self.n == 0
            || self.rev > MAX_REV
            || !plain(&self.actor, MAX_ACTOR)
            || !plain(&self.key_prefix, MAX_KEY_PREFIX)
            || !plain(&self.tool, MAX_TOOL)
        {
            return None;
        }
        let written = self.written.min(MAX_COUNT);
        let commands = self.commands.clamp(1, MAX_COUNT).max(written);
        let who = self
            .reviewed_by
            .filter(|w| plain(w, MAX_ACTOR))
            .zip(self.reviewed_at_ms);
        Some(Header {
            n: self.n,
            rev: self.rev,
            actor: self.actor,
            key_prefix: self.key_prefix,
            tool: self.tool,
            commands,
            written,
            atomic: self.atomic,
            first_seq: self.first_seq,
            last_seq: self.last_seq.max(self.first_seq),
            at_ms: self.at_ms,
            note: self.note.as_deref().and_then(Note::clean),
            review: self.review,
            reviewed_by: who.clone().map(|(w, _)| w),
            reviewed_at_ms: who.map(|(_, at)| at),
        })
    }
}
