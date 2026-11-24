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

mod log;
mod names;
mod plan;
pub mod scan;
mod text;
pub mod value;

/// The colour reader lives in `meldframe-css` (shared with the HTML importer); its path here stays what it was.
pub use meldframe_css::color;

use meldframe_check::Limits;
use meldframe_module::{Module, Registrar, RiskTier};
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
    // A byte order mark that an editor or a download put in front of the text is not part of the first selector.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    Ok(plan::build(&scan::scan(text), &options.limits))
}

/// The name of the agent tool. (The tool keeps the name the epic gave it; the file calls what it makes variables.)
pub const TOOL: &str = "import_tokens";

/// The tool text an agent reads (English, part of the contract; `docs/mcp-tools.json` lists it).
pub const DESCRIPTION: &str = "Import design variables from CSS text that you read from the user's project: the custom properties of :root, of a rule that picks the dark mode (.dark, [data-theme=dark], a prefers-color-scheme: dark query) and of a Tailwind v4 @theme block. Nothing is executed or fetched, the text is only read, so resolve a tailwind.config.js, an @import or a url() yourself first and send the CSS text (css, at most 256 KiB, counted in bytes). The file gets a collection theme (the @theme variables, one mode, default) and a collection root (the :root variables, with the modes light and, when a dark rule sets any, dark); a value that is only var(--x) becomes an alias of that variable; a name is lowercase and a Tailwind namespace is split off with a slash (--color-brand-500 is color/brand-500). A hex colour is kept as written; rgb, hsl, oklch and oklab are converted to hex and a colour outside sRGB is clamped; rem is 16 px. Shadows, fonts, strings, computed values (calc, clamp, color-mix) and anything else the model cannot hold are skipped. The report lists every conversion and every skip with its reason under entries (at most 200 lines and 32 KiB of text in the entries, the JSON around them is more; omitted counts the rest): read it before you bind a node. The answer to part 1 and to a dryRun carries the whole report; the answers to later parts carry only its counts (the entries are those of the first part). The commands are written as apply_commands writes them, under the same limits, quota and undo, atomic and idempotent. A call takes at most 100 commands, so a long import comes in parts: the answer says parts, and you send the same call again with part 2, then 3, and so on, with the same idempotencyKey and css (each part is atomic, the whole import is not; send them in order). dryRun true writes nothing and returns the report and the number of parts. The import adds, and its collections are always root and theme (ids col-root and col-theme), so a second CSS file imported into the same file is refused as E_DUPLICATE_ID: join the files into one text. Old comma forms such as hsl(120, 50, 50) are accepted although browsers refuse them. Sending text whose variables the file already holds is refused as E_DUPLICATE_ID (to finish an import that stopped part way, send the same call again; to change what a file holds, use apply_commands). The call counts as one op for each command against the workspace's ops per minute (6,000), so an import of 5,000 variables uses most of a minute: a part that the quota refuses writes nothing, so wait and send that part again. Every call needs an idempotencyKey (also accepted as idempotency_key); the ops of a call are named by the key and the text, so send the same call again after a timeout and nothing is written twice (replayed is true), while the same key with other css is a new call. Text over the limit is E_IMPORT_TOO_LARGE.";

/// The variables import module. It registers one Change-tier agent tool: an import writes variables to the file.
#[derive(Debug, Clone, Copy)]
pub struct ImportModule;

impl Module for ImportModule {
    fn name(&self) -> &'static str {
        "variables-import"
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.agent_tool(TOOL, DESCRIPTION, RiskTier::Change);
    }
}
