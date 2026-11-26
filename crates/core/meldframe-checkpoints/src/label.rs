//! The label of a manual checkpoint and the actor strings a row carries.

use crate::MAX_LABEL_CHARS;
use meldframe_activity::{MAX_ACTOR, MAX_COMBINING_RUN, is_combining_mark, is_hidden_text};
use meldframe_text::TextError;

/// A label: the person's plain text on a `manual` checkpoint. Trimmed, never empty, at most [`MAX_LABEL_CHARS`]
/// characters, with no line break, no control character, no hidden character (the stricter set `is_hidden_text` of
/// `meldframe-activity`, not the one of node names) and no run of more than [`MAX_COMBINING_RUN`] combining marks.
/// The text is kept as sent: nothing is parsed and no link is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label(String);

/// Why a label was refused. Every one is `invalid`: nothing is written, and nothing is cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelError {
    /// Over [`MAX_LABEL_CHARS`] characters.
    TooLong,
    /// The first control or hidden character, the line break among them.
    Hidden(char),
    /// A run of more than [`MAX_COMBINING_RUN`] combining marks.
    TooManyMarks,
}

impl LabelError {
    pub fn code(&self) -> &'static str {
        "invalid"
    }
}

impl std::fmt::Display for LabelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LabelError::TooLong => write!(f, "the label is over {MAX_LABEL_CHARS} characters"),
            LabelError::Hidden(c) => write!(
                f,
                "the label holds a line break, a control or a hidden character (U+{:04X}): a label is one line of plain text",
                *c as u32
            ),
            LabelError::TooManyMarks => write!(
                f,
                "the label has more than {MAX_COMBINING_RUN} combining marks in a row"
            ),
        }
    }
}

impl std::error::Error for LabelError {}

impl Label {
    /// The server's check of a label. The text is looked at as sent, so a hidden character or a line break at an
    /// edge is refused and a trim cannot remove one quietly; then it is trimmed. Nothing left is `None` (an empty
    /// label is the same as none). A bad label is refused, never cut: a silent edit would hide what was written.
    pub fn parse(text: &str) -> Result<Option<Label>, LabelError> {
        if let Some(c) = text.chars().find(|c| is_hidden_text(*c)) {
            return Err(LabelError::Hidden(c));
        }
        let text = text.trim();
        if text.is_empty() {
            return Ok(None);
        }
        if text.chars().count() > MAX_LABEL_CHARS {
            return Err(LabelError::TooLong);
        }
        let mut run = 0;
        for c in text.chars() {
            run = if is_combining_mark(c) { run + 1 } else { 0 };
            if run > MAX_COMBINING_RUN {
                return Err(LabelError::TooManyMarks);
            }
        }
        Ok(Some(Label(text.to_owned())))
    }

    /// The `label` field of a request: no field and an empty one are both no label.
    pub fn parse_arg(text: Option<&str>) -> Result<Option<Label>, LabelError> {
        text.map_or(Ok(None), Label::parse)
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why an actor string was refused (`invalid`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorError {
    Empty,
    /// Over [`MAX_ACTOR`] bytes.
    TooLong,
    /// A control or hidden character.
    Hidden(char),
    /// A run of more than [`MAX_COMBINING_RUN`] combining marks.
    TooManyMarks,
}

impl ActorError {
    pub fn code(&self) -> &'static str {
        "invalid"
    }
}

impl std::fmt::Display for ActorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActorError::Empty => write!(f, "the actor is empty"),
            ActorError::TooLong => write!(f, "the actor is over {MAX_ACTOR} bytes"),
            ActorError::Hidden(c) => {
                write!(
                    f,
                    "the actor holds a control or hidden character (U+{:04X})",
                    *c as u32
                )
            }
            ActorError::TooManyMarks => write!(
                f,
                "the actor has more than {MAX_COMBINING_RUN} combining marks in a row"
            ),
        }
    }
}

impl std::error::Error for ActorError {}

/// The rule of an actor string in a row: the rule of `meldframe_text::actor` (not empty, at most [`MAX_ACTOR`] bytes, no
/// control or hidden character, no run of more than [`MAX_COMBINING_RUN`] combining marks), so a row and a batch header
/// refuse the same actors.
pub fn check_actor(actor: &str) -> Result<(), ActorError> {
    meldframe_text::actor(actor).map_err(|e| match e {
        TextError::Empty => ActorError::Empty,
        TextError::Hidden(c) => ActorError::Hidden(c),
        TextError::TooManyMarks => ActorError::TooManyMarks,
        _ => ActorError::TooLong,
    })
}
