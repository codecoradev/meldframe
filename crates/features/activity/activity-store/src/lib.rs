//! The store of batch headers (E5.1, ADR-0021): the numbered batches of a file, each with its note and review state,
//! kept beside the op log and never in it.
//!
//! - A header is named by workspace, file, actor and key prefix (the batch of ADR-0016). [`BatchStore::record`] makes
//!   it on the first call that wrote ops of the prefix and updates it on later ones (a retry, the next part of an
//!   import): the number `n` is per file, from 1, never reused and given in the order headers are made.
//! - `rev` is one counter per file that orders every change of a header. A record that changes nothing and a review
//!   set to the state it has take no `rev`.
//! - `commands` and `written` only rise (and `written` never passes `commands`: a part of an import that brings it
//!   past raises `commands` to it), `last_seq` only rises, `first_seq`, `at_ms`, `tool` and `atomic` stay as
//!   the first call set them, the first note wins and a review survives a later record.
//! - The caps are the store's, so every adapter enforces them the same way, also when calls run at once: [`Caps`]
//!   (per actor in a file, per file, per workspace). A cap refuses a new header and never an update of one.
//! - A store never reaches another workspace's or another file's headers.
//!
//! [`BatchStore`] is the seam; [`MemoryBatches`] keeps the headers in memory, `PgStore` in Postgres
//! (`meldframe-store-pg`). With the cargo feature `contract`, [`contract`] is the set of cases every adapter passes.

#[cfg(feature = "contract")]
pub mod contract;
mod store;

pub use store::MemoryBatches;

use meldframe_activity::{
    ACTOR_HEADERS, FILE_HEADERS, Header, MAX_ACTOR, MAX_COUNT, MAX_KEY_PREFIX, MAX_PAGE,
    MAX_PER_ACTOR, MAX_PER_FILE, MAX_PER_WORKSPACE, MAX_TOOL, NewBatch, Review, WORKSPACE_HEADERS,
    is_hidden_text,
};

/// Every stable code a store call can fail with. `invalid` and `unavailable` are shared with the other tools.
pub const CODES: [&str; 4] = ["invalid", "activity_full", "not_found", "unavailable"];

/// The caps on the headers a new batch may make. The default is the constants of `meldframe-activity`; a test builds
/// a store with small ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    pub per_actor: usize,
    pub per_file: usize,
    pub per_workspace: usize,
}

impl Default for Caps {
    fn default() -> Self {
        Caps {
            per_actor: MAX_PER_ACTOR,
            per_file: MAX_PER_FILE,
            per_workspace: MAX_PER_WORKSPACE,
        }
    }
}

/// Why a call failed. [`ActivityError::code`] is the stable code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityError {
    /// The input is wrong: a name, a count or a page size that does not fit its rule.
    Invalid(String),
    /// A cap is reached; `what` names it.
    Full { limit: usize, what: &'static str },
    /// The file has no batch with that number (or it is in another workspace).
    NotFound(u32),
    /// The store failed.
    Unavailable(String),
}

impl ActivityError {
    pub fn code(&self) -> &'static str {
        match self {
            ActivityError::Invalid(_) => "invalid",
            ActivityError::Full { .. } => "activity_full",
            ActivityError::NotFound(_) => "not_found",
            ActivityError::Unavailable(_) => "unavailable",
        }
    }

    /// Whether the same call can succeed later unchanged: only when the store failed.
    pub fn retryable(&self) -> bool {
        matches!(self, ActivityError::Unavailable(_))
    }
}

impl std::fmt::Display for ActivityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActivityError::Invalid(why) => write!(f, "{why}"),
            ActivityError::Full { limit, what } => {
                write!(f, "at most {limit} {what}: the limit is reached")
            }
            ActivityError::NotFound(n) => write!(f, "this file has no batch {n}"),
            ActivityError::Unavailable(_) => write!(f, "the activity store is unavailable"),
        }
    }
}

impl std::error::Error for ActivityError {}

/// What a call says about the note it carried, for the answer: `kept` (stored now), `kept_earlier` (the batch had a
/// note already, and the first non-empty note wins) or `none` (the call carried no note).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteOutcome {
    Kept,
    KeptEarlier,
    None,
}

impl NoteOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            NoteOutcome::Kept => "kept",
            NoteOutcome::KeptEarlier => "kept_earlier",
            NoteOutcome::None => "none",
        }
    }
}

/// What [`BatchStore::record`] answers: the header as it is now, read back from the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub header: Header,
    /// Whether this call made the header (took a number).
    pub created: bool,
    /// Whether the header changed (a new one, or a rise, or a note): only then it took a `rev` and has a frame.
    pub changed: bool,
    pub note: NoteOutcome,
    /// How many headers of the file are not `accepted`, as of this call.
    pub unreviewed: u32,
}

/// What [`BatchStore::set_review`] answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reviewed {
    pub header: Header,
    /// False when the state was the one the header had: nothing changed and no `rev` was taken.
    pub changed: bool,
    pub unreviewed: u32,
}

/// Which headers a list call returns: those with `after < n < before` (both exclusive, both optional), the newest
/// `limit` of them, newest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Query {
    pub before: Option<u32>,
    pub after: Option<u32>,
    pub limit: usize,
}

/// A page of headers, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub headers: Vec<Header>,
    /// Whether more headers match the query, older than the last one returned.
    pub has_more: bool,
    /// How many headers of the whole file are not `accepted` (the query does not narrow it).
    pub unreviewed: u32,
}

/// The seam: keep the batch headers. Methods take `&self` and a store is `Sync`: calls run at once, and a number is
/// never given twice. Every method is atomic. Callers check their input first; a store checks it again, so no store
/// holds an unchecked header.
pub trait BatchStore: Send + Sync {
    /// Make the header of `batch` (identity: its actor and key prefix in the file) or update it; see the module text
    /// for what an update changes. Refused with `Full` when a new header would pass a cap.
    fn record(
        &self,
        workspace: &str,
        file: &str,
        batch: &NewBatch,
    ) -> Result<Recorded, ActivityError>;

    /// A page of the file's headers, newest first; a file with none is an empty page, not an error.
    fn list(&self, workspace: &str, file: &str, query: &Query) -> Result<Page, ActivityError>;

    /// Set the review state of batch `n` and who set it and when (`at_ms`, the server clock). Setting the state the
    /// header has changes nothing. Refused with `NotFound` for a number the file does not have.
    fn set_review(
        &self,
        workspace: &str,
        file: &str,
        n: u32,
        state: Review,
        by: &str,
        at_ms: u64,
    ) -> Result<Reviewed, ActivityError>;
}

fn invalid(why: impl Into<String>) -> ActivityError {
    ActivityError::Invalid(why.into())
}

/// An actor string: 1 to [`MAX_ACTOR`] bytes, no control or hidden character.
pub fn check_actor(actor: &str) -> Result<(), ActivityError> {
    if actor.is_empty() || actor.len() > MAX_ACTOR || actor.chars().any(is_hidden_text) {
        return Err(invalid(format!(
            "an actor is 1 to {MAX_ACTOR} bytes without control or hidden characters"
        )));
    }
    Ok(())
}

/// What a store refuses before it writes: a name that does not fit, a count out of range, sequence numbers that run
/// backwards.
pub fn check_batch(b: &NewBatch) -> Result<(), ActivityError> {
    check_actor(&b.actor)?;
    if b.key_prefix.is_empty()
        || b.key_prefix.chars().count() > MAX_KEY_PREFIX
        || b.key_prefix.chars().any(is_hidden_text)
    {
        return Err(invalid(format!(
            "a key prefix is 1 to {MAX_KEY_PREFIX} characters without control or hidden characters"
        )));
    }
    if b.tool.is_empty() || b.tool.len() > MAX_TOOL || b.tool.chars().any(is_hidden_text) {
        return Err(invalid(format!(
            "a tool name is 1 to {MAX_TOOL} bytes without control or hidden characters"
        )));
    }
    if b.commands == 0 || b.commands > MAX_COUNT || b.written > b.commands {
        return Err(invalid(format!(
            "a batch has 1 to {MAX_COUNT} commands and has written at most that many"
        )));
    }
    if b.first_seq > b.last_seq {
        return Err(invalid("first_seq is above last_seq"));
    }
    // The database holds these as bigint: a larger value is refused, never cut.
    if [b.first_seq, b.last_seq, b.at_ms]
        .iter()
        .any(|v| *v > MAX_DB_INT)
    {
        return Err(invalid(format!(
            "a sequence number or a time is at most {MAX_DB_INT}"
        )));
    }
    Ok(())
}

/// The largest sequence number or time a header holds: what a Postgres `bigint` holds.
pub const MAX_DB_INT: u64 = i64::MAX as u64;

/// A page size: 1 to [`MAX_PAGE`].
pub fn check_query(q: &Query) -> Result<(), ActivityError> {
    if q.limit == 0 || q.limit > MAX_PAGE {
        return Err(invalid(format!("a page is 1 to {MAX_PAGE} batches")));
    }
    Ok(())
}

/// The cap a new header of `actor` would pass, if any, given how many headers `actor` has in the file, the file has
/// and the workspace has. The same order in every adapter: actor, file, workspace.
pub fn cap_reached(
    caps: &Caps,
    actor: usize,
    file: usize,
    workspace: usize,
) -> Option<ActivityError> {
    let full = |limit, what| Some(ActivityError::Full { limit, what });
    if actor >= caps.per_actor {
        full(caps.per_actor, ACTOR_HEADERS)
    } else if file >= caps.per_file {
        full(caps.per_file, FILE_HEADERS)
    } else if workspace >= caps.per_workspace {
        full(caps.per_workspace, WORKSPACE_HEADERS)
    } else {
        None
    }
}

/// What an update of a header by a later call of its batch does, for every adapter: `commands` rises to the larger
/// count, `written` rises by what the call newly wrote (at most [`MAX_COUNT`]), `last_seq` rises, a note is kept only
/// when the header has none (the first non-empty note wins), and nothing else changes. Returns whether the header
/// changed (then it needs a new `rev`) and what became of the call's note. `rev` is the caller's to set.
pub fn merge(h: &mut Header, b: &NewBatch) -> (bool, NoteOutcome) {
    let before = (h.commands, h.written, h.last_seq, h.note.is_some());
    h.commands = h.commands.max(b.commands);
    h.written = h.written.saturating_add(b.written).min(MAX_COUNT);
    // The parts of one import share a prefix and each says how many commands it has: written never passes commands.
    h.commands = h.commands.max(h.written);
    h.last_seq = h.last_seq.max(b.last_seq);
    let note = match (&h.note, &b.note) {
        (_, None) => NoteOutcome::None,
        (Some(_), Some(_)) => NoteOutcome::KeptEarlier,
        (None, Some(n)) => {
            h.note = Some(n.clone());
            NoteOutcome::Kept
        }
    };
    (
        before != (h.commands, h.written, h.last_seq, h.note.is_some()),
        note,
    )
}

/// A header made by the first call of its batch, with the number and `rev` the store took.
pub fn new_header(n: u32, rev: u64, b: &NewBatch) -> Header {
    Header {
        n,
        rev,
        actor: b.actor.clone(),
        key_prefix: b.key_prefix.clone(),
        tool: b.tool.clone(),
        commands: b.commands,
        written: b.written,
        atomic: b.atomic,
        first_seq: b.first_seq,
        last_seq: b.last_seq,
        at_ms: b.at_ms,
        note: b.note.clone(),
        review: Review::Unreviewed,
        reviewed_by: None,
        reviewed_at_ms: None,
    }
}
