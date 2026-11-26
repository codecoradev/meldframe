//! The note of a batch. The hidden-text rule and the combining marks are the one set of `meldframe-text`, which
//! this crate re-exports (`is_hidden_text`, `is_combining_mark`).

use crate::{MAX_COMBINING_RUN, MAX_NOTE_BREAKS, MAX_NOTE_CHARS};
use meldframe_text::{first_hidden, mark_run_exceeds};
pub use meldframe_text::{is_combining_mark, is_hidden_text};

/// A note: the plain text an agent leaves on a batch. It is trimmed, not empty, at most [`MAX_NOTE_CHARS`]
/// characters and [`MAX_NOTE_BREAKS`] line breaks (`\n`), with no hidden character and no run of more than
/// [`MAX_COMBINING_RUN`] combining marks. The text is kept as sent; no markup is parsed and no link is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note(String);

/// Why a note was refused. Every one is `invalid`: the call is refused whole and nothing is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteError {
    /// Nothing is left after trimming.
    Empty,
    /// Over [`MAX_NOTE_CHARS`] characters.
    TooLong,
    /// Over [`MAX_NOTE_BREAKS`] line breaks.
    TooManyBreaks,
    /// The first hidden character or control character (the line break apart).
    Hidden(char),
    /// A run of more than [`MAX_COMBINING_RUN`] combining marks.
    TooManyMarks,
}

impl NoteError {
    /// The stable code of the refusal.
    pub fn code(&self) -> &'static str {
        "invalid"
    }
}

impl std::fmt::Display for NoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NoteError::Empty => write!(f, "the note is empty: leave it out or write something"),
            NoteError::TooLong => write!(f, "the note is over {MAX_NOTE_CHARS} characters"),
            NoteError::TooManyBreaks => {
                write!(f, "the note has more than {MAX_NOTE_BREAKS} line breaks")
            }
            NoteError::Hidden(c) => write!(
                f,
                "the note holds a control or hidden character (U+{:04X}): a note is plain text, with line breaks as the only control character",
                *c as u32
            ),
            NoteError::TooManyMarks => write!(
                f,
                "the note has more than {MAX_COMBINING_RUN} combining marks in a row"
            ),
        }
    }
}

impl std::error::Error for NoteError {}

/// `\r\n` read as `\n`.
fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

impl Note {
    /// The server's check of a note, before anything is written: `\r\n` read as `\n`; no control character but the
    /// line break and no hidden character (an edge is checked too, so a trim cannot remove one quietly); trimmed; not
    /// empty; at most [`MAX_NOTE_CHARS`] characters (Unicode scalar values) and [`MAX_NOTE_BREAKS`] line breaks; no run
    /// of more than [`MAX_COMBINING_RUN`] combining marks. A bad note is refused, never cut: silent edits of what an
    /// agent wrote would hide what it tried to say.
    pub fn parse(text: &str) -> Result<Note, NoteError> {
        let text = lf(text);
        if let Some(c) = first_hidden(&text, true) {
            return Err(NoteError::Hidden(c));
        }
        let text = text.trim();
        if text.is_empty() {
            return Err(NoteError::Empty);
        }
        if text.chars().count() > MAX_NOTE_CHARS {
            return Err(NoteError::TooLong);
        }
        if text.matches('\n').count() > MAX_NOTE_BREAKS {
            return Err(NoteError::TooManyBreaks);
        }
        if mark_run_exceeds(text, MAX_COMBINING_RUN) {
            return Err(NoteError::TooManyMarks);
        }
        Ok(Note(text.to_owned()))
    }

    /// The `note` argument of a call: no argument and an empty string are both no note; a blank one is
    /// [`NoteError::Empty`], as [`Note::parse`] has it.
    pub fn parse_arg(text: Option<&str>) -> Result<Option<Note>, NoteError> {
        match text {
            None | Some("") => Ok(None),
            Some(t) => Note::parse(t).map(Some),
        }
    }

    /// The app layer's second check: what [`Note::parse`] would refuse is cut instead, because a header arrives from
    /// a socket and a route and the app layer trusts neither. Hidden characters and controls go (a line break stays,
    /// `\r\n` is one), marks past [`MAX_COMBINING_RUN`] in a row go (counted once the hidden characters between them
    /// are gone), line breaks past [`MAX_NOTE_BREAKS`] become spaces, the text is cut to [`MAX_NOTE_CHARS`]
    /// characters and trimmed (the ends first, so the breaks around the text are not counted). Nothing left is `None`. A checked note comes back unchanged.
    pub fn clean(text: &str) -> Option<Note> {
        // Hidden characters go first, then the ends are trimmed, so the breaks around the text are not counted.
        let shown: String = lf(text)
            .chars()
            .filter(|c| *c == '\n' || !is_hidden_text(*c))
            .collect();
        let (mut out, mut run, mut breaks) = (String::new(), 0, 0);
        for c in shown.trim().chars() {
            run = if is_combining_mark(c) { run + 1 } else { 0 };
            if run > MAX_COMBINING_RUN {
                continue;
            }
            if c == '\n' {
                breaks += 1;
            }
            out.push(if c == '\n' && breaks > MAX_NOTE_BREAKS {
                ' '
            } else {
                c
            });
        }
        // A cut at the length can leave a space or a break at the end, so the trim comes last.
        let out: String = out.trim().chars().take(MAX_NOTE_CHARS).collect();
        let out = out.trim();
        (!out.is_empty()).then(|| Note(out.to_owned()))
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Note {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
