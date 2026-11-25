//! The mapping of structure, layout, sizing and text (E4.4, slice 4; `docs/import-html.md` section 3, ADR-0024): HTML
//! text and optional extra CSS to the Commands that add its nodes, and a report of what was not read. It is a function
//! of the text: nothing is opened, fetched or run, no clock is read, and the work is counted (`Report::work`).
//!
//! [`plan`] reads the text with the tree builder, computes the style of every element, and maps the elements to nodes:
//! a frame for a container, a text for a line of text, a rect for a placeholder, in the order of the text. The Commands
//! are `addNode`s, a parent before its children, with no `pos` (the server places each node after the last sibling).
//! The first Command adds the root. The plan does not enforce the batch limit of 100 Commands (the tool of slice 7
//! does, as `E_IMPORT_HTML_TOO_MANY`): [`Plan::exceeds_batch`] says whether it passes it.

mod build;
mod item;
mod layout;
mod names;
mod page;
mod parts;
mod plain;
mod pre;
mod refuse;
mod size;
mod text;

pub use item::{Approx, Asset, PlannedNode, Unsupported};

use crate::css::{self, style_tree};
use crate::html::{self, ParseError, Report, parse};
use build::Builder;
use build::put;
use item::{Item, Notes, blank, flatten};
use layout::{Ctx, Flow, set_padding};
use meldframe_layout::{StubMeasurer, TextMeasurer};
use meldframe_protocol::{Command, LayoutMode, NodeKind, SizeMode};
use names::name_of;

/// What the call may set.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// The width of the root frame when the source gives none: 1440, the Desktop preset.
    pub width: f64,
    /// The name of the root frame. `None`: the `<title>`, else "Imported HTML".
    pub name: Option<String>,
    /// The ids are `{id_prefix}-{i}` for the `i`th node. The tool sets a prefix from the key and the text.
    pub id_prefix: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            width: 1440.0,
            name: None,
            id_prefix: "imp".into(),
        }
    }
}

/// The most bytes of HTML and extra CSS together (`docs/import-html.md` section 2.1).
pub const MAX_INPUT_BYTES: usize = 256 * 1024;
/// The most Commands one batch holds (`batchLimit` of `docs/mcp-tools.json`).
pub const BATCH_LIMIT: usize = 100;

/// The result of a plan.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The `addNode` Commands, the root first, a parent always before its children.
    pub commands: Vec<Command>,
    /// What there is to say about each node: `nodes[i]` is the node `commands[i]` adds.
    pub nodes: Vec<PlannedNode>,
    /// The images the source names, as written and not fetched.
    pub assets: Vec<Asset>,
    /// What the tree builder, the CSS stages and the mapping left out or repaired.
    pub report: Report,
}

impl Plan {
    /// Does the plan pass the batch limit.
    pub fn exceeds_batch(&self) -> bool {
        self.commands.len() > BATCH_LIMIT
    }
}

/// Map `html` (a fragment or a whole document) and the extra `css` to Commands.
///
/// Refused as [`ParseError`]: more than 256 KiB of `html` and `css` together, more than the limit of elements, or no
/// element that makes a node. Anything else the source holds is mapped or listed in the report, never an error.
pub fn plan(html: &str, css: &str, options: &Options) -> Result<Plan, ParseError> {
    plan_with(html, css, options, &StubMeasurer)
}

/// Like [`plan`], measuring text with `measurer` (the stored sizes of what hugs).
pub fn plan_with(
    html: &str,
    css: &str,
    options: &Options,
    measurer: &dyn TextMeasurer,
) -> Result<Plan, ParseError> {
    let total = html.len() + css.len();
    if total > MAX_INPUT_BYTES {
        return Err(ParseError::TooLarge {
            bytes: total,
            max: MAX_INPUT_BYTES,
        });
    }
    let limits = html::Limits {
        max_input_bytes: MAX_INPUT_BYTES - css.len(),
        ..html::Limits::default()
    };
    let parsed = parse(html, &limits)?;
    let (tree, mut report) = (parsed.tree, parsed.report);
    let styles = style_tree(&tree, css, &css::Limits::default(), &mut report);
    let pre = pre::pre(&tree, &styles, &mut report);
    let page = page::find(&tree, &styles, &pre, &mut report);
    let mut b = Builder {
        tree: &tree,
        styles: &styles,
        pre: &pre,
        report: &mut report,
        pending: Notes::default(),
    };
    // The width of the root: the page's, else the call's. A page with no height hugs, as a page does.
    let width = page.width.unwrap_or(options.width);
    let ctx = Ctx {
        flow: Flow::Column,
        stretch: true,
        sw: SizeMode::Fixed,
        sh: if page.height.is_some() {
            SizeMode::Fixed
        } else {
            SizeMode::Hug
        },
        px: (Some(width), page.height),
    };
    let mut kids = Vec::new();
    b.content(page.container, &ctx, &mut kids);
    if kids.is_empty() {
        return Err(ParseError::Empty);
    }
    let root = if page.is_plain() && kids.len() == 1 && kids[0].node.kind == NodeKind::Frame {
        let mut only = kids.remove(0);
        // The call's name wins; then the root's own `data-name` (an export imports back with its name); then the
        // title; then what the element is called.
        if let Some(n) = options
            .name
            .clone()
            .or(page.title.clone().filter(|_| !only.named))
        {
            only.node.name = n;
        }
        // The root has no parent: it is as wide as it says, else as the call says, and it never fills.
        let n = &mut only.node;
        n.absolute = false;
        if n.sw == SizeMode::Fill {
            (n.sw, n.w) = (SizeMode::Fixed, width);
        }
        if n.sh == SizeMode::Fill {
            n.sh = SizeMode::Hug;
        }
        only
    } else {
        let mut node = blank(NodeKind::Frame);
        node.layout = LayoutMode::Column;
        node.name = options
            .name
            .clone()
            .or(page.title)
            .unwrap_or_else(|| "Imported HTML".into());
        set_padding(&mut node, page.pad);
        put(&mut node, &page.paint);
        node.w = width;
        if let Some(h) = page.height {
            node.h = h;
        } else {
            node.sh = SizeMode::Hug;
        }
        let el = tree.root();
        Item {
            node,
            kids,
            source: name_of(el),
            at: 0,
            notes: page.notes,
            src: None,
            named: false,
        }
    };
    let mut flat = flatten(root, &options.id_prefix);
    store_sizes(&mut flat.commands, measurer);
    Ok(Plan {
        commands: flat.commands,
        nodes: flat.nodes,
        assets: flat.assets,
        report,
    })
}

/// The most a stored size may be (`meldframe_check::Limits::max_size`).
const MAX_STORED: f64 = 100_000.0;

/// The size of a node that hugs or fills is the layout's, not the stylesheet's: the layout core works it out over the
/// nodes the Commands add, with `measurer` for the text, and it is stored in `w` and `h` (a fixed size is left as the page
/// said). The layout orders siblings by `pos`, which the Commands do not carry (the server places each node after the
/// last one); with none it falls back on the id, which changes no size, only the place of a node.
fn store_sizes(commands: &mut [Command], measurer: &dyn TextMeasurer) {
    let nodes: Vec<meldframe_protocol::Node> = commands
        .iter()
        .filter_map(|c| match c {
            Command::AddNode { node, .. } => Some(node.clone()),
            _ => None,
        })
        .collect();
    let rects = meldframe_layout::compute_rects_indexed(&nodes, measurer);
    for (i, r) in rects {
        if let Some(Command::AddNode { node, .. }) = commands.get_mut(i) {
            if node.sw != SizeMode::Fixed {
                node.w = r.w.clamp(0.0, MAX_STORED);
            }
            if node.sh != SizeMode::Fixed {
                node.h = r.h.clamp(0.0, MAX_STORED);
            }
        }
    }
}
