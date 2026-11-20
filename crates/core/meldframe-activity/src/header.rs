//! The batch header and the review state.

use crate::Note;

/// A person's mark on a batch. It has nothing to do with an accepted op of the room or with an Approval of the
/// glossary: it says only that a person looked at the batch and kept it, or asked for a closer look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Review {
    /// Nobody has decided yet: the default of every new header.
    #[default]
    Unreviewed,
    /// A person kept the batch.
    Accepted,
    /// A person flagged the batch for a closer look.
    NeedsReview,
}

impl Review {
    /// The stable name, as the wire and the column spell it.
    pub fn as_str(self) -> &'static str {
        match self {
            Review::Unreviewed => "unreviewed",
            Review::Accepted => "accepted",
            Review::NeedsReview => "needs_review",
        }
    }

    /// The state a name stands for; any other text is `None`.
    pub fn parse(name: &str) -> Option<Self> {
        [Review::Unreviewed, Review::Accepted, Review::NeedsReview]
            .into_iter()
            .find(|r| r.as_str() == name)
    }

    /// Whether a header in this state counts in `unreviewedCount`: every state but `accepted`.
    pub fn is_open(self) -> bool {
        self != Review::Accepted
    }
}

/// What a tool call tells the store about the batch it wrote (or tried to write): the input of `record`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewBatch {
    pub actor: String,
    /// The key prefix of the call: its ops are `{key_prefix}:{i}`.
    pub key_prefix: String,
    pub tool: String,
    /// How many commands the batch is said to have (a retry may say more; only a higher count is kept).
    pub commands: u32,
    /// How many ops this call newly wrote (a replay writes none). It is added to the header's `written` unless
    /// `known` is given.
    pub written: u32,
    /// How many ops of this batch's prefix the log holds, as this call saw it (a call of one prefix reads it from its
    /// own answer or from the log). When given, the header's `written` becomes the larger of what it has and this, and
    /// `written` above adds nothing: two calls of one batch that overlap (a replay and a delayed original) count each
    /// op once, and a retry that fails again still raises the count. `None` for the parts of an import, whose keys no
    /// one call sees whole, so each part adds what it wrote.
    pub known: Option<u32>,
    /// Whether the call was atomic.
    pub atomic: bool,
    /// The lowest and the highest sequence number among the ops this call wrote.
    pub first_seq: u64,
    pub last_seq: u64,
    /// The server clock, in milliseconds.
    pub at_ms: u64,
    pub note: Option<Note>,
}

/// A batch header as the store keeps it. `n` is per file, from 1 and never reused; `rev` is one counter per file
/// that orders every change of a header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub n: u32,
    pub rev: u64,
    pub actor: String,
    pub key_prefix: String,
    pub tool: String,
    /// The largest number of commands the batch was said to have; it only rises.
    pub commands: u32,
    /// How many of its ops are known to be in the log; it only rises. A count kept beside the log: when the two
    /// disagree the log wins.
    pub written: u32,
    pub atomic: bool,
    pub first_seq: u64,
    pub last_seq: u64,
    pub at_ms: u64,
    pub note: Option<Note>,
    pub review: Review,
    /// Who set the review state and when; both are set together, or neither.
    pub reviewed_by: Option<String>,
    pub reviewed_at_ms: Option<u64>,
}
