//! The note of a batch and the hidden-text rule.

use crate::{MAX_COMBINING_RUN, MAX_NOTE_BREAKS, MAX_NOTE_CHARS};

/// Whether a character changes how a text reads without showing, or is not text at all.
///
/// This is stricter than `meldframe_mcp_defs::is_invisible` (which covers U+200B to 200F, U+202A to 202E, U+2028,
/// U+2029, U+2066 to 2069 and U+FEFF and nothing else) and named in full: the controls (`char::is_control`, the
/// line break among them: a note allows `\n` by its own rule, a node name gets no such exception), U+00AD, U+034F,
/// U+061C, U+115F and U+1160, U+17B4 and U+17B5, U+180B to 180F, U+200B to 200F, U+2028 to 202E, U+2060 to 206F,
/// U+2800, U+3164, U+FE00 to FE0F, U+FEFF, U+FFA0, U+FFF9 to FFFB, U+1BCA0 to 1BCA3, U+1D173 to 1D17A, U+E0000 to
/// E007F and U+E0100 to E01EF.
pub fn is_hidden_text(c: char) -> bool {
    c.is_control()
        || matches!(c,
            '\u{00AD}' | '\u{034F}' | '\u{061C}' | '\u{115F}'..='\u{1160}' | '\u{17B4}'..='\u{17B5}'
            | '\u{180B}'..='\u{180F}' | '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}' | '\u{2800}' | '\u{3164}' | '\u{FE00}'..='\u{FE0F}' | '\u{FEFF}'
            | '\u{FFA0}' | '\u{FFF9}'..='\u{FFFB}' | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}' | '\u{E0000}'..='\u{E007F}' | '\u{E0100}'..='\u{E01EF}')
}

/// Whether a character is a combining mark of the blocks a note caps: U+0300 to 036F, 1AB0 to 1AFF, 1DC0 to 1DFF,
/// 20D0 to 20FF and FE20 to FE2F.
pub fn is_combining_mark(c: char) -> bool {
    matches!(c,
        '\u{0300}'..='\u{036F}' | '\u{1AB0}'..='\u{1AFF}' | '\u{1DC0}'..='\u{1DFF}'
        | '\u{20D0}'..='\u{20FF}' | '\u{FE20}'..='\u{FE2F}')
}

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
        if let Some(c) = text.chars().find(|c| *c != '\n' && is_hidden_text(*c)) {
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
        let mut run = 0;
        for c in text.chars() {
            run = if is_combining_mark(c) { run + 1 } else { 0 };
            if run > MAX_COMBINING_RUN {
                return Err(NoteError::TooManyMarks);
            }
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
