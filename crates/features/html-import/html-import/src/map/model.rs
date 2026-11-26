//! What a caller gives an import and gets back: the options, the result, the node rows and the reasons an import makes nothing.

use super::notes::{Change, Lost};
use crate::css::Report;
use crate::html::ParseError;
use meldframe_protocol::{Command, NodeKind};

/// The most commands one import makes: the batch limit of the tools (`docs/mcp-tools.json`, `batchLimit`).
pub const MAX_COMMANDS: usize = 100;

/// The most bytes of HTML and CSS together, counted in bytes (`docs/import-html.md` section 2.1).
pub const MAX_INPUT_BYTES: usize = 256 * 1024;

/// The most `skipped` lines an import lists; the count of the rest is [`Imported::skipped_omitted`].
pub const MAX_SKIPPED: usize = 200;

/// What the caller decides about an import.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// The ids are `{id_prefix}-{i}` for the command `i` (0 is the root): the caller makes them unique, 1 to 56
    /// characters of letters, digits and `-`.
    pub id_prefix: String,
    /// The name of the root frame; absent, the one of the page, else "Imported HTML".
    pub name: Option<String>,
    /// The width of the root frame when the page gives none.
    pub width: f64,
    /// Where the root frame sits (the caller places it).
    pub origin: (f64, f64),
    /// The frame of the file the root goes into; absent, the root is a top-level frame.
    pub parent: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            id_prefix: "imp".into(),
            name: None,
            width: 1440.0,
            origin: (0.0, 0.0),
            parent: None,
        }
    }
}

/// Why an import makes nothing. Each has a stable code (`docs/error-codes.md`, "HTML import codes").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// HTML and CSS are over [`MAX_INPUT_BYTES`] together.
    TooLarge { bytes: usize, max: usize },
    /// The HTML makes more elements than the tree builder holds.
    TooManyElements { max: usize, at: usize },
    /// No element makes a node.
    Empty,
    /// The page makes more nodes than one call takes.
    TooMany { nodes: usize, max: usize },
}

impl ImportError {
    pub fn code(&self) -> &'static str {
        match self {
            ImportError::TooLarge { .. } => "E_IMPORT_HTML_TOO_LARGE",
            ImportError::TooManyElements { .. } => "E_IMPORT_HTML_TOO_MANY_ELEMENTS",
            ImportError::Empty => "E_IMPORT_HTML_EMPTY",
            ImportError::TooMany { .. } => "E_IMPORT_HTML_TOO_MANY",
        }
    }
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::TooLarge { bytes, max } => write!(
                f,
                "the HTML and the CSS are {bytes} bytes together and an import reads at most {max}: send one section at a time"
            ),
            ImportError::TooManyElements { max, at } => write!(
                f,
                "the HTML has more than {max} elements (the one at byte {at} is past the limit): send one section at a time"
            ),
            ImportError::Empty => f.write_str(
                "no element of the HTML makes a node (it is empty, only scripts or styles, or everything is display: none)",
            ),
            ImportError::TooMany { nodes, max } => write!(
                f,
                "the HTML makes at least {nodes} nodes and one import writes at most {max}: send one section at a time, each under the parentId of the frame it belongs in"
            ),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<ParseError> for ImportError {
    fn from(e: ParseError) -> Self {
        match e {
            ParseError::TooLarge { bytes, max } => ImportError::TooLarge { bytes, max },
            ParseError::TooMany { max, at } => ImportError::TooManyElements { max, at },
            ParseError::Empty => ImportError::Empty,
        }
    }
}

/// What the importer says about one node it made. `source` names the element it came from (`section.hero#2`: the
/// tag, its first class, and the place of the element in the text), so that an agent can match what it sent to what
/// it got.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeInfo {
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    pub source: String,
    /// Values changed to the nearest the model holds.
    pub approximated: Vec<Change>,
    /// Properties that were not carried (counted by `strict`).
    pub unsupported: Vec<Lost>,
    /// How many lines the `approximated` list would hold past its bound of 32 (0 for almost every node).
    pub approx_omitted: usize,
    /// The same for `unsupported`.
    pub lost_omitted: usize,
}

/// An element that made no node: `display_none` or `no_model_equivalent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub source: String,
    pub code: &'static str,
}

/// An address that is listed for the agent and never followed. `usage` is `image` (an `img`) or `background`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRef {
    pub node: String,
    pub usage: &'static str,
    pub src: String,
}

/// The result of an import.
#[derive(Debug, Clone)]
pub struct Imported {
    /// One `addNode` for each node, a parent before its children, siblings in the order of the source.
    pub commands: Vec<Command>,
    /// The id of the root frame.
    pub root: String,
    /// One row for each command, in the same order.
    pub nodes: Vec<NodeInfo>,
    pub skipped: Vec<Skipped>,
    pub skipped_omitted: usize,
    pub assets: Vec<AssetRef>,
    /// What the readers said: the repairs, the cuts, the refused selectors and declarations, the listed classes.
    pub report: Report,
}

impl Imported {
    /// How many properties were not carried, over all the nodes, those a full list left out included: what `strict`
    /// refuses.
    pub fn unsupported_count(&self) -> usize {
        self.nodes
            .iter()
            .map(|n| n.unsupported.len() + n.lost_omitted)
            .sum()
    }
}
