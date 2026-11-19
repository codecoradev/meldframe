//! Variables import feature module (E3.4): CSS custom properties and Tailwind v4 `@theme` text to the commands that add
//! collections, modes, variables and aliases to a file (ADR-0020), with a report of what was converted or skipped.
//!
//! [`import_css`] is a pure function from text to commands. It runs no code and opens nothing: `@import` and `url()`
//! are reported as skipped and never followed, a value that is computed (`calc()`, `color-mix()`) is skipped and never
//! evaluated, and the size of the text and the number of what it makes are bounded. The commands are the ones of
//! `meldframe-protocol`, in an order in which each can be applied after the ones before it (collections first, an
//! alias after the variable it points at), so the caller can send them as one atomic batch through the same gate as any
//! other command.
//!
//! What it makes: `@theme` variables go to the collection `theme` (one mode, `default`); custom properties of `:root`
//! go to the collection `root`, whose modes are `light` (the root rule) and `dark` (a rule that picks the dark mode,
//! such as `.dark`, `[data-theme=dark]` or a `prefers-color-scheme: dark` query); a pure `var(--x)` value is an alias.
//! A colour that is not hex is converted to hex and the report says so; one that sRGB cannot show is clamped and the
//! report says that too. Nothing is changed without a line in the report.

pub mod color;
mod log;
mod names;
mod plan;
pub mod scan;
mod text;
pub mod value;

use meldframe_check::Limits;
use meldframe_protocol::Command;

/// The most bytes of text an import reads.
pub const MAX_INPUT_BYTES: usize = 256 * 1024;

/// The most bytes of text a report lists in all (names, messages and codes of its entries, the notes at the top aside).
pub const MAX_REPORT_BYTES: usize = 32 * 1024;

/// The bytes of text the report helpers have cut (a test reads it; nothing else does).
#[doc(hidden)]
pub static TEXT_WORK: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The most entries a report lists in all; the count of what was left out is in [`Report::omitted`].
pub const MAX_ENTRIES: usize = 200;

/// What an import is asked for.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// The most bytes of text; more is [`ImportError::TooLarge`].
    pub max_bytes: usize,
    /// The ceilings of a file: names, ids, numbers and how many collections, modes and variables.
    pub limits: Limits,
}

impl Default for ImportOptions {
    fn default() -> Self {
        ImportOptions {
            max_bytes: MAX_INPUT_BYTES,
            limits: Limits::default(),
        }
    }
}

/// Why an import did not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The text is longer than the limit.
    TooLarge { bytes: usize, max: usize },
}

impl ImportError {
    /// The stable code (`docs/error-codes.md`).
    pub fn code(&self) -> &'static str {
        match self {
            ImportError::TooLarge { .. } => "E_IMPORT_TOO_LARGE",
        }
    }
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::TooLarge { bytes, max } => write!(
                f,
                "the text is {bytes} bytes and an import reads at most {max}: send the style sheet in parts"
            ),
        }
    }
}

impl std::error::Error for ImportError {}

/// What kind of line a report entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// Imported, but not as written: a colour in another form, `rem`, a name that was normalised.
    Converted,
    /// Not imported, with the reason.
    Skipped,
    /// Something to know that is neither.
    Note,
}

impl EntryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EntryKind::Converted => "converted",
            EntryKind::Skipped => "skipped",
            EntryKind::Note => "note",
        }
    }
}

/// One line of the report.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub kind: EntryKind,
    /// A stable reason: `converted`, `clipped`, `rem`, `renamed`, `import_skipped`, `computed_value`...
    pub code: &'static str,
    /// The custom property, selector or at-rule it is about.
    pub name: String,
    pub message: String,
}

/// What the import made and what it did not.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    pub collections: usize,
    pub modes: usize,
    pub variables: usize,
    pub colors: usize,
    pub numbers: usize,
    pub aliases: usize,
    /// Custom properties that were not imported.
    pub skipped: usize,
    /// At most [`MAX_ENTRIES`] lines.
    pub entries: Vec<Entry>,
    /// Lines left out of `entries` because it was full.
    pub omitted: usize,
}

/// The commands of an import and its report.
#[derive(Debug, Clone)]
pub struct Import {
    pub commands: Vec<Command>,
    pub report: Report,
}

/// Read `text` as CSS and make the commands that add what it defines. An empty `commands` means nothing in the text
/// could be imported; the report says why.
pub fn import_css(text: &str, options: &ImportOptions) -> Result<Import, ImportError> {
    if text.len() > options.max_bytes {
        return Err(ImportError::TooLarge {
            bytes: text.len(),
            max: options.max_bytes,
        });
    }
    Ok(plan::build(&scan::scan(text), &options.limits))
}
