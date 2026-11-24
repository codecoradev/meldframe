//! The bounds of the tree builder (ADR-0024) and the reasons it refuses a text.

/// What a read is bounded by. A text over `max_input_bytes`, or with more than `max_elements` elements, is refused;
/// the others cut something and the report says what (see [`crate::html::Report`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// The most bytes of text (`docs/import-html.md` counts HTML and CSS together; the caller passes what is left for the HTML).
    /// More is [`ParseError::TooLarge`], before anything is read. 256 KiB.
    pub max_input_bytes: usize,
    /// The most elements the tree holds (scripts and what the depth limit flattens are not counted). One more is
    /// [`ParseError::TooMany`]. 5,000, a starting guess with nothing behind it.
    pub max_elements: usize,
    /// The deepest nesting of elements. An element that would be deeper is not made: its content goes into the
    /// deepest element that was. 64, the `MAX_DEPTH` of the export.
    pub max_depth: usize,
    /// The most attributes an element keeps; later ones are left out. 64, a starting guess.
    pub max_attributes: usize,
    /// The most bytes of an attribute value, as written. A longer one is left out with its name. 1,000, the
    /// `MAX_VALUE` of the variables import.
    pub max_value_bytes: usize,
    /// The most bytes of one text (a run of text and what sits between its comments); more is cut. A `<style>`
    /// is bounded by the input and not by this. 64 KiB, a starting guess.
    pub max_text_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_input_bytes: 256 * 1024,
            max_elements: 5_000,
            max_depth: 64,
            max_attributes: 64,
            max_value_bytes: 1_000,
            max_text_bytes: 64 * 1024,
        }
    }
}

/// Why a text was not read into a tree. Malformed HTML is not one of them: the builder recovers and says so in
/// the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The text is longer than the limit.
    TooLarge { bytes: usize, max: usize },
    /// The text makes more elements than the limit; `at` is the byte where the one that passed it starts.
    TooMany { max: usize, at: usize },
    /// The text holds no element: it is empty, plain text, comments, a doctype, or only scripts.
    Empty,
}

impl ParseError {
    /// The stable code (ADR-0024; the rows of `docs/error-codes.md` come with the tool).
    pub fn code(&self) -> &'static str {
        match self {
            ParseError::TooLarge { .. } => "E_IMPORT_HTML_TOO_LARGE",
            ParseError::TooMany { .. } => "E_IMPORT_HTML_TOO_MANY_ELEMENTS",
            ParseError::Empty => "E_IMPORT_HTML_EMPTY",
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::TooLarge { bytes, max } => write!(
                f,
                "the HTML is {bytes} bytes and an import reads at most {max}: send one section at a time"
            ),
            ParseError::TooMany { max, at } => write!(
                f,
                "the HTML has more than {max} elements (the one at byte {at} is past the limit): send one section at a time"
            ),
            ParseError::Empty => f.write_str("the HTML has no element to import"),
        }
    }
}

impl std::error::Error for ParseError {}
