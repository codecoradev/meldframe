//! The refusing checks: `plain` for any short string, and the rules of a name and a key on top of it.

use crate::charset::{first_hidden, mark_run_exceeds};
use crate::{MAX_ACTOR, MAX_COMBINING_RUN, MAX_FILE_NAME, MAX_KEY, MAX_KEY_BYTES};

/// Why a text was refused. A caller words its own error from this; [`std::fmt::Display`] says the reason as a phrase
/// that goes after the thing named ("the actor is empty", "the key holds a control or hidden character").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextError {
    /// Nothing at all.
    Empty,
    /// Only white space.
    Blank,
    /// `.` or `..`, which a file name may not be.
    DotName,
    /// More than this many characters (Unicode scalar values).
    TooManyChars(usize),
    /// More than this many bytes.
    TooManyBytes(usize),
    /// The first hidden character or control character (see [`crate::is_hidden_text`]).
    Hidden(char),
    /// A run of more than [`MAX_COMBINING_RUN`] combining marks.
    TooManyMarks,
}

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextError::Empty => write!(f, "is empty"),
            TextError::Blank => write!(f, "is only whitespace"),
            TextError::DotName => write!(f, "is a dot name"),
            TextError::TooManyChars(n) => write!(f, "is longer than {n} characters"),
            TextError::TooManyBytes(n) => write!(f, "is longer than {n} bytes"),
            TextError::Hidden(c) => write!(
                f,
                "holds a control or hidden character (U+{:04X})",
                *c as u32
            ),
            TextError::TooManyMarks => write!(
                f,
                "has more than {MAX_COMBINING_RUN} combining marks in a row"
            ),
        }
    }
}

impl std::error::Error for TextError {}

/// The strict check of a short string that is shown or kept as an identifier: not empty, at most `max_chars`
/// characters and at most `max_bytes` bytes (both are checked, because the two counts differ for a text that is not
/// ASCII, and a caller that passes only one of them leaves the other open: pass `usize::MAX` for a bound that does
/// not apply), no control or hidden character ([`crate::is_hidden_text`]) and no run of more than
/// [`MAX_COMBINING_RUN`] combining marks. The first failing rule is the one reported, in that order: empty, bytes,
/// characters (the byte count is a length read, so a huge text is refused before it is walked), hidden, marks.
pub fn plain(s: &str, max_chars: usize, max_bytes: usize) -> Result<(), TextError> {
    if s.is_empty() {
        return Err(TextError::Empty);
    }
    if s.len() > max_bytes {
        return Err(TextError::TooManyBytes(max_bytes));
    }
    if s.chars().count() > max_chars {
        return Err(TextError::TooManyChars(max_chars));
    }
    if let Some(c) = first_hidden(s, false) {
        return Err(TextError::Hidden(c));
    }
    if mark_run_exceeds(s, MAX_COMBINING_RUN) {
        return Err(TextError::TooManyMarks);
    }
    Ok(())
}

fn name(s: &str, dots: bool) -> Result<(), TextError> {
    if s.is_empty() {
        Err(TextError::Empty)
    } else if s.len() > MAX_FILE_NAME {
        Err(TextError::TooManyBytes(MAX_FILE_NAME))
    } else if s.trim().is_empty() {
        Err(TextError::Blank)
    } else if dots && (s == "." || s == "..") {
        Err(TextError::DotName)
    } else {
        plain(s, MAX_FILE_NAME, MAX_FILE_NAME)
    }
}

/// A workspace name: [`plain`] within [`MAX_FILE_NAME`] bytes, and not only white space.
pub fn workspace_name(s: &str) -> Result<(), TextError> {
    name(s, false)
}

/// A file name: as [`workspace_name`], and not `.` or `..`. A transport that also refuses a slash adds that rule
/// itself.
pub fn file_name(s: &str) -> Result<(), TextError> {
    name(s, true)
}

/// An actor string: [`plain`] within [`MAX_ACTOR`] bytes (and so within [`MAX_ACTOR`] characters). The bytes are the
/// limit because the stores hold an actor in a column bounded in bytes.
pub fn actor(s: &str) -> Result<(), TextError> {
    plain(s, MAX_ACTOR, MAX_ACTOR)
}

/// The prefix of the actor string of an agent, as the lanes, the layer rows and the checkpoint run rule read it. It says
/// who an actor is for display only; it decides nothing about access.
pub const AGENT_PREFIX: &str = "agent:";
/// The prefix of the actor string of a person.
pub const PERSON_PREFIX: &str = "person:";

/// Whether an actor string is an agent's: it starts with [`AGENT_PREFIX`]. The one definition: the lane kind of the
/// activity panel and the run rule of the checkpoints call it, so the two cannot read an actor differently.
pub fn is_agent(actor: &str) -> bool {
    actor.starts_with(AGENT_PREFIX)
}

/// An idempotency key: [`plain`] within [`MAX_KEY`] characters (and [`MAX_KEY_BYTES`] bytes).
pub fn key(s: &str) -> Result<(), TextError> {
    plain(s, MAX_KEY, MAX_KEY_BYTES)
}
