//! Export feature module (E4.1): a frame of a file as HTML with plain CSS or Tailwind v4 utilities.
//!
//! [`export_frame`] is a pure function. It reads the same render step as the canvas (`meldframe_render::resolve_with`:
//! nodes, variables and the modes picked, to painted boxes) for everything the canvas paints, the node for the fields
//! that layout reads, and the layout core for the size of text and of a fill that its siblings squeeze. A bound
//! field is `var(--...)` (ADR-0019), the document is the contract of `docs/fidelity-contract.md`, and whatever the
//! export cannot carry is in the [`Report`]. The table of `meldframe_render::coverage` says which node fields it
//! renders, and the tests of this crate check the table against the output.
//!
//! The HTML is a whole document. In [`Format::Css`] the stylesheet is inline and every rule selects
//! `[data-node="<id>"]`; in [`Format::Tailwind`] the elements carry utilities and the document links
//! [`ExportOptions::css_href`], which is [`Export::css`] after Tailwind has built it: that text is the input for the
//! Tailwind v4 compiler (the variables, the font and the reset) and needs the utilities of the HTML added.

mod decl;
mod emit;
mod names;
mod report;
mod tw;
mod vars;

pub use report::{Finding, Report};

use meldframe_layout::TextMeasurer;
use meldframe_module::{Module, Registrar, RiskTier};
use meldframe_protocol::{Node, Variables};
use meldframe_render::ModeChoice;

/// The most bytes an export may have (HTML and stylesheet together) unless the caller asks for another limit.
pub const MAX_OUTPUT_BYTES: usize = 256 * 1024;

/// The style language of the export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// One rule per node, `[data-node="<id>"]{...}`, inline in the document.
    Css,
    /// Tailwind v4 utilities on each element, arbitrary values where the contract names no utility.
    Tailwind,
}

/// What an export is asked for.
#[allow(missing_debug_implementations)]
pub struct ExportOptions<'a> {
    pub format: Format,
    /// The mode picked for each collection (view state, as on the canvas); the others show their default mode.
    pub modes: ModeChoice,
    /// Measures text: pinned into the CSS as `min-width` and `line-height` (rule G4).
    pub measurer: &'a dyn TextMeasurer,
    /// Where the font file is, relative to the document.
    pub font_href: String,
    /// Where the stylesheet is, relative to the document ([`Format::Tailwind`] only).
    pub css_href: String,
    /// A prefix for the asset id in `url(...)`; the ids the document uses are in [`Report::assets`].
    pub asset_base: String,
    /// The most bytes of HTML and stylesheet together; more is [`ExportError::TooLarge`].
    pub max_bytes: usize,
}

impl<'a> ExportOptions<'a> {
    pub fn new(format: Format, measurer: &'a dyn TextMeasurer) -> Self {
        ExportOptions {
            format,
            modes: ModeChoice::default(),
            measurer,
            font_href: "inter.woff2".into(),
            css_href: "meldframe.css".into(),
            asset_base: "assets/".into(),
            max_bytes: MAX_OUTPUT_BYTES,
        }
    }

    pub fn modes(mut self, modes: ModeChoice) -> Self {
        self.modes = modes;
        self
    }
}

/// The result: the document, the stylesheet, and what could not be carried.
#[derive(Debug, Clone)]
pub struct Export {
    pub html: String,
    pub css: String,
    pub report: Report,
}

/// Why an export is refused. Each has a stable code (`docs/error-codes.md`, "Export codes").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
    /// The id is not a node of the file.
    NotFound,
    /// The node is not a frame.
    NotAFrame,
    /// The output is larger than the limit.
    TooLarge { bytes: usize, max: usize },
}

impl ExportError {
    pub fn code(&self) -> &'static str {
        match self {
            ExportError::NotFound => "E_NODE_NOT_FOUND",
            ExportError::NotAFrame => "E_NOT_A_FRAME",
            ExportError::TooLarge { .. } => "E_EXPORT_TOO_LARGE",
        }
    }
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::NotFound => f.write_str("no node has that id"),
            ExportError::NotAFrame => f.write_str("only a frame can be exported"),
            ExportError::TooLarge { bytes, max } => write!(
                f,
                "the export is {bytes} bytes and the limit is {max}: export a smaller frame"
            ),
        }
    }
}

impl std::error::Error for ExportError {}

/// Export frame `frame` and everything inside it. A top-level frame sits at its own x and y; a frame inside another
/// sits at the origin. Pure and deterministic: the same file gives the same bytes.
pub fn export_frame(
    nodes: &[Node],
    vars: &Variables,
    frame: &str,
    options: &ExportOptions<'_>,
) -> Result<Export, ExportError> {
    emit::export(nodes, vars, frame, options)
}

/// The tool text an agent reads (English, part of the contract; `docs/mcp-tools.json` lists it).
pub const DESCRIPTION: &str = "Export a frame of a file, and everything inside it, as a whole HTML document with the styles of a design that matches the canvas. Pass file and frame (the id of a frame; a top-level frame sits at its own x and y, a frame inside another at the origin). format is css (the default: one rule per node in an inline stylesheet) or tailwind (Tailwind v4 utilities on each element; the css in the result is the input of the Tailwind compiler, and the html links it as meldframe.css). Variables the frame uses become CSS variables (--collection-name) on :root, one rule per extra mode, and a bound fill, radius or opacity is var(--...); pass modes, an object of collection id to mode id, to export a mode other than the default (the result says it in the html attributes). The result lists under report what the export could not carry: unsupported (a shader is approximated by a gradient, a border or image on text is dropped, a border on a frame that clips is clipped) and warnings (layout findings, bindings that do not resolve, text a browser breaks differently), and the ids of the images the html refers to under assets (the files are not included; the font is inter.woff2 next to the document). Text sizes come from the server's stub measurer, not the browser's, so text boxes are only as exact as that. A frame whose export is over the size limit is refused as E_EXPORT_TOO_LARGE: export a smaller frame; a node that is not in the file is E_NODE_NOT_FOUND and one that is not a frame E_NOT_A_FRAME. Changes nothing.";

/// The name of the agent tool.
pub const TOOL: &str = "export_frame";

/// The export module. It registers one Read-tier agent tool: an export changes nothing.
#[derive(Debug, Clone, Copy)]
pub struct ExportModule;

impl Module for ExportModule {
    fn name(&self) -> &'static str {
        "export"
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.agent_tool(TOOL, DESCRIPTION, RiskTier::Read);
    }
}
