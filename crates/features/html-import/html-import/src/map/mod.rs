//! The mapping (E4.4, `docs/import-html.md` sections 3.1 to 3.5 and 4.3): the styled tree to a list of `addNode`
//! commands, with the report of what each node could not take. [`import`] is the whole importer as a pure function of
//! the text: it parses, computes the styles, maps, and bounds the result at [`MAX_COMMANDS`].
//!
//! - **Structure** (`walk.rs`). An element is a box (a node) or part of a run of text. A `display: none` element, and
//!   the ones a browser never draws, make nothing; `display: contents` and inline elements pass their content to the
//!   element they are in. The text of a run is one `text` node with the style of the box that holds it (the model has
//!   one style per text: a different inline style is listed on the node). An element whose content is one run and
//!   that paints nothing of its own is that text node; with a background, border, shadow or padding it is a frame with
//!   a text in it. A wrapper that only holds one box and says nothing of its own makes no node.
//! - **Layout and sizing** (`size.rs`, `node.rs`). A flex container is a row or a column frame, a block is a column
//!   with no gap, and a frame whose children are all absolute has no layout. A size is fixed, hug or fill by the
//!   rules of section 3.4 (rule G1: no fill under a parent that hugs on that axis). The fields the model defaults
//!   differently from CSS are always written (section 3.1).
//! - **Text** (`text.rs`). Spaces collapse as the browser collapses them, `br` is a line break, and the text of a node
//!   is file content: it is kept as written, never cleaned. Names the importer makes up, and everything the report
//!   echoes, go through `meldframe-text` (rule U9).
//! - **Paint** is the paint stage (`crate::paint`), read once for each box.
//!
//! The result is bounded: more than [`MAX_COMMANDS`] nodes is [`ImportError::TooMany`] and nothing is made. Nothing is
//! fetched or run; the stage is a function of the text and builds for `wasm32-unknown-unknown`.

mod items;
mod leaf;
mod names;
mod node;
mod notes;
mod page;
mod refuse;
mod size;
mod text;
mod walk;

use crate::css::{Limits as CssLimits, Report, style_tree};
use crate::html::{Limits, ParseError, parse};
use meldframe_protocol::{Command, NodeKind};
pub use notes::{Change, Lost};

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
                "the HTML makes {nodes} nodes and one import writes at most {max}: send one section at a time, each under the parentId of the frame it belongs in"
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
    /// How many properties were not carried, over all the nodes: what `strict` refuses.
    pub fn unsupported_count(&self) -> usize {
        self.nodes.iter().map(|n| n.unsupported.len()).sum()
    }
}

/// Read `html`, with `css` as one more style sheet, into nodes. The cascade reads `css` after every `<style>`.
pub fn import(html: &str, css: &str, options: &Options) -> Result<Imported, ImportError> {
    let bytes = html.len() + css.len();
    if bytes > MAX_INPUT_BYTES {
        return Err(ImportError::TooLarge {
            bytes,
            max: MAX_INPUT_BYTES,
        });
    }
    let limits = Limits {
        max_input_bytes: MAX_INPUT_BYTES - css.len(),
        ..Limits::default()
    };
    let parsed = parse(html, &limits)?;
    let mut report = parsed.report;
    let styles = style_tree(&parsed.tree, css, &CssLimits::default(), &mut report);
    let mut walk = walk::Walk::new(&parsed.tree, &styles, &mut report, options);
    let root = walk.page().ok_or(ImportError::Empty)?;
    let (skipped, skipped_omitted) = walk.into_skipped();
    flatten(root, options, report, skipped, skipped_omitted)
}

/// The tree of specs as a list of commands: parent first, ids from the prefix, the root at the place and in the
/// frame the caller says.
fn flatten(
    root: node::Spec,
    options: &Options,
    report: Report,
    skipped: Vec<Skipped>,
    skipped_omitted: usize,
) -> Result<Imported, ImportError> {
    let nodes = root.count();
    if nodes > MAX_COMMANDS {
        return Err(ImportError::TooMany {
            nodes,
            max: MAX_COMMANDS,
        });
    }
    let mut out = Imported {
        commands: Vec::with_capacity(nodes),
        root: String::new(),
        nodes: Vec::with_capacity(nodes),
        skipped,
        skipped_omitted,
        assets: Vec::new(),
        report,
    };
    let mut todo = vec![(root, None::<String>)];
    // Depth first with the first child on top, so siblings keep the order of the source.
    while let Some((mut spec, parent)) = todo.pop() {
        let id = format!("{}-{}", options.id_prefix, out.commands.len());
        spec.node.id.clone_from(&id);
        match &parent {
            Some(p) => spec.node.parent_id = Some(p.clone()),
            None => {
                spec.node.parent_id.clone_from(&options.parent);
                (spec.node.x, spec.node.y) = options.origin;
                out.root.clone_from(&id);
            }
        }
        out.assets
            .extend(spec.assets.iter().map(|(usage, a)| AssetRef {
                node: id.clone(),
                usage,
                src: a.src.clone(),
            }));
        out.nodes.push(NodeInfo {
            id: id.clone(),
            name: spec.node.name.clone(),
            kind: spec.node.kind,
            source: spec.source,
            approximated: spec.notes.approx,
            unsupported: spec.notes.lost,
        });
        let kids = std::mem::take(&mut spec.kids);
        out.commands.push(Command::AddNode {
            node: spec.node,
            index: None,
        });
        todo.extend(kids.into_iter().rev().map(|k| (k, Some(id.clone()))));
    }
    Ok(out)
}
