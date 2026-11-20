//! The rules of the activity panel (E5.1, ADR-0021), with no I/O: what a batch header holds, what a note may say,
//! which characters are hidden text, the review state of a batch and every bound.
//!
//! A batch is still the written ops of one actor whose keys are `{prefix}:{n}` (ADR-0016). Its header is a row kept
//! beside the log, never an op: it holds only what the log cannot tell (its number, its note, how many commands it
//! had, who reviewed it). The store that keeps headers is `activity-store`; this crate is what the server, the store
//! and the app layer all agree on, so it depends on nothing.
//!
//! The note is the agent's plain text on a batch. [`Note::parse`] is the server's check (a bad note refuses the whole
//! call) and [`Note::clean`] is the app layer's second check (it cuts what the first would refuse, from every header
//! it takes in, because a header arrives from a socket and a route and the app layer trusts neither).

mod header;
mod note;
mod sink;

pub use header::{Header, NewBatch, Review};
pub use note::{Note, NoteError, is_combining_mark, is_hidden_text};
pub use sink::{BatchCall, BatchSink, Marked};

/// The longest note, in characters (Unicode scalar values): the owner's limit, as a critique item of `design_review`.
pub const MAX_NOTE_CHARS: usize = 500;
/// The most line breaks a note holds.
pub const MAX_NOTE_BREAKS: usize = 3;
/// The longest run of combining marks in a note: more would let stacked marks smear a line.
pub const MAX_COMBINING_RUN: usize = 4;

/// The longest key prefix of a batch: the longest idempotency key (`meldframe-mcp-defs`, a test compares them).
pub const MAX_KEY_PREFIX: usize = 100;
/// The longest actor string, in bytes.
pub const MAX_ACTOR: usize = 128;
/// The longest tool name, in bytes.
pub const MAX_TOOL: usize = 64;
/// The largest `commands` and `written` of a header: an import's ceiling is 20,000 commands, a call takes 100.
pub const MAX_COUNT: u32 = 100_000;

/// The most headers one actor has in one file.
pub const MAX_PER_ACTOR: usize = 2_000;
/// The most headers one file has.
pub const MAX_PER_FILE: usize = 20_000;
/// The most headers one workspace has.
pub const MAX_PER_WORKSPACE: usize = 200_000;
/// The most headers one list call returns.
pub const MAX_PAGE: usize = 100;
/// The headers a list call returns when it names no limit.
pub const DEFAULT_PAGE: usize = 50;

/// What a `Full` refusal counts, for the message.
pub const ACTOR_HEADERS: &str = "batch headers of one actor in a file";
pub const FILE_HEADERS: &str = "batch headers in a file";
pub const WORKSPACE_HEADERS: &str = "batch headers in a workspace";
