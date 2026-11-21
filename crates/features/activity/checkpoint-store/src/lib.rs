//! The store of checkpoints (E5.3, ADR-0023): the numbered rows of a file, kept beside the op log and never in it.
//!
//! - A row is named by workspace and file. `n` is per file, from 1, never reused (a roll deletes rows and the counter
//!   goes on) and given in the order rows are made; a list is newest `n` first and needs no paging, because the shares
//!   of [`meldframe_checkpoints`] bound a file to 100 rows.
//! - The rules are `meldframe_checkpoints::roll` and [`plan`]: a `manual` row is refused at its share or its actor's cap
//!   (`checkpoint_full`) and never rolled out, the automatic kinds roll inside their own share and are never refused.
//!   The rule is one pure function over the rows of the file, so every adapter plans the same way and differs only in
//!   where it keeps the rows and how it holds the lock.
//! - A request with a key is unique per (file, actor, key): the same request again returns its row and writes nothing,
//!   a different one is `key_reused` with the first row's number. A run start is unique per (file, actor, `seq`).
//! - A store never reaches another workspace's or another file's rows.
//!
//! [`CheckpointStore`] is the seam; [`MemoryCheckpoints`] keeps the rows in memory, `PgStore` in Postgres
//! (`meldframe-store-pg`). With the cargo feature `contract`, [`contract`] is the set of cases every adapter passes.

#[cfg(feature = "contract")]
pub mod contract;
mod store;

pub use store::MemoryCheckpoints;

use meldframe_activity::is_hidden_text;
use meldframe_checkpoints::{Checkpoint, Kind, NewCheckpoint, Refusal, Rolled, roll};

/// The longest key of a request, in bytes (the limit of an op key).
pub const MAX_KEY: usize = 100;

/// Every stable code a store call can fail with. `invalid` and `unavailable` are shared with the other tools.
pub const CODES: [&str; 4] = ["invalid", "checkpoint_full", "key_reused", "unavailable"];

/// Why a call failed. [`CheckpointError::code`] is the stable code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointError {
    /// The input is wrong: a key that does not fit its rule.
    Invalid(String),
    /// A `manual` row over its share or its actor's cap; `what` names it.
    Full { limit: usize, what: &'static str },
    /// The key was used by a different request; the number is the first row's.
    KeyReused(u32),
    /// The store failed.
    Unavailable(String),
}

impl CheckpointError {
    pub fn code(&self) -> &'static str {
        match self {
            CheckpointError::Invalid(_) => "invalid",
            CheckpointError::Full { .. } => "checkpoint_full",
            CheckpointError::KeyReused(_) => "key_reused",
            CheckpointError::Unavailable(_) => "unavailable",
        }
    }

    /// Whether the same call can succeed later unchanged: only when the store failed.
    pub fn retryable(&self) -> bool {
        matches!(self, CheckpointError::Unavailable(_))
    }
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckpointError::Invalid(why) => write!(f, "{why}"),
            CheckpointError::Full { limit, what } => {
                write!(f, "at most {limit} {what}: the limit is reached")
            }
            CheckpointError::KeyReused(n) => {
                write!(f, "this key made checkpoint {n}, a different request")
            }
            CheckpointError::Unavailable(_) => write!(f, "the checkpoint store is unavailable"),
        }
    }
}

impl std::error::Error for CheckpointError {}

impl From<Refusal> for CheckpointError {
    fn from(r: Refusal) -> Self {
        match r {
            Refusal::Full { limit, what } => CheckpointError::Full { limit, what },
            Refusal::Counter => {
                CheckpointError::Unavailable("the counter of the file is not usable".into())
            }
        }
    }
}

/// What [`CheckpointStore::create`] answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    /// The row, read back from the store: the one this call made, or the one an earlier identical call made.
    pub checkpoint: Checkpoint,
    /// Whether this call made the row (took a number); false for a retry of a key or of a run start.
    pub created: bool,
    /// The numbers of the rows this call rolled out of the file.
    pub rolled: Vec<u32>,
}

/// The seam. A call blocks, so the server runs it on the blocking pool.
pub trait CheckpointStore: std::fmt::Debug + Send + Sync {
    /// Make a row of `new` in a file, at the server's clock `at_ms`, or answer the row an earlier identical call made.
    /// `key` is the request's idempotency key (a person's call has one; a run start has none).
    fn create(
        &self,
        workspace: &str,
        file: &str,
        key: Option<&str>,
        new: &NewCheckpoint,
        at_ms: u64,
    ) -> Result<Made, CheckpointError>;

    /// The rows of a file, newest `n` first. A file with none, and another workspace's, is empty.
    fn list(&self, workspace: &str, file: &str) -> Result<Vec<Checkpoint>, CheckpointError>;
}

/// A key: not empty, at most [`MAX_KEY`] bytes, no control or hidden character.
pub fn check_key(key: &str) -> Result<(), CheckpointError> {
    if key.is_empty() || key.len() > MAX_KEY {
        return Err(CheckpointError::Invalid(format!(
            "the key is empty or over {MAX_KEY} bytes"
        )));
    }
    match key.chars().find(|c| is_hidden_text(*c)) {
        Some(c) => Err(CheckpointError::Invalid(format!(
            "the key holds a control or hidden character (U+{:04X})",
            c as u32
        ))),
        None => Ok(()),
    }
}

/// A row of a file with the key it was made with, as an adapter holds it.
#[derive(Debug, Clone, Copy)]
pub struct Held<'a> {
    pub row: &'a Checkpoint,
    pub key: Option<&'a str>,
}

/// What a create comes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// The request was made before: this row answers it and nothing is written.
    Existing(u32),
    /// A new row, with the rows that roll out.
    Insert(Rolled),
}

/// The plan of a create in a file that holds `held` (all of its rows) with the counter at `next_n`, decided in this
/// order so every adapter answers alike: the key is checked (a run start has none), a key already used by this actor
/// answers its row or is `key_reused`, a run start already taken for its actor and `seq` answers its row, and
/// otherwise [`roll`] decides, with `checkpoint_full` for a `manual` row the file cannot take.
pub fn plan(
    held: &[Held<'_>],
    next_n: u32,
    key: Option<&str>,
    new: &NewCheckpoint,
) -> Result<Plan, CheckpointError> {
    match (key, new.kind()) {
        (Some(_), Kind::RunStart) => {
            return Err(CheckpointError::Invalid(
                "a run start is taken by the server and has no key".into(),
            ));
        }
        (Some(k), _) => check_key(k)?,
        (None, _) => {}
    }
    if let Some(k) = key
        && let Some(h) = held
            .iter()
            .find(|h| h.key == Some(k) && h.row.by.as_deref() == new.by())
    {
        return if same_request(h.row, new) {
            Ok(Plan::Existing(h.row.n))
        } else {
            Err(CheckpointError::KeyReused(h.row.n))
        };
    }
    if new.kind() == Kind::RunStart
        && let Some(h) = held.iter().find(|h| {
            h.row.kind == Kind::RunStart
                && h.row.about.as_deref() == new.about()
                && h.row.seq == new.seq()
        })
    {
        return Ok(Plan::Existing(h.row.n));
    }
    let rows: Vec<Checkpoint> = held.iter().map(|h| h.row.clone()).collect();
    Ok(Plan::Insert(roll(&rows, next_n, new)?))
}

/// Whether a row is what a request asks for: the kind, the `seq`, the label and the actor it is about.
fn same_request(row: &Checkpoint, new: &NewCheckpoint) -> bool {
    row.kind == new.kind()
        && row.seq == new.seq()
        && row.label.as_ref() == new.label()
        && row.about.as_deref() == new.about()
}
