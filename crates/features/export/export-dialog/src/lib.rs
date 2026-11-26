//! The Export dialog (D9), the parts that read no `App`: the frame an export acts on, and the HTML built for it. The
//! export is the `export` feature's, run on the person's own view of the file (pending ops included, the modes on
//! view). Text is measured by the measurer the caller gives: the app layer gives the one its host injected (the browser's
//! Pixi metrics on the font the canvas draws with), so a hug text exports the width the canvas draws, and the layout
//! core's stub when the host gave none. The MCP tool `export_frame` runs on the server, which has no fonts: it keeps the stub.

mod variables;

pub use variables::variables_dialog;

use export::{ExportError, ExportOptions, Finding, Format, MAX_OUTPUT_BYTES};
use meldframe_app_model::{ExportDialog, ExportFinding, ExportFormat, ExportTarget};
use meldframe_layout::TextMeasurer;
use meldframe_protocol::{Node, NodeKind, Variables};
use meldframe_text::{MAX_LABEL, label};
use meldframe_variables::ModeChoice;
use std::collections::BTreeMap;

/// The frame an export acts on: the one selected node, when it is a frame.
pub fn target(selection: &[String], nodes: &[Node]) -> Option<ExportTarget> {
    match selection {
        [id] => nodes
            .iter()
            .find(|n| n.id == *id && n.kind == NodeKind::Frame)
            .map(|n| ExportTarget {
                id: n.id.clone(),
                name: label(&n.name, MAX_LABEL),
            }),
        _ => None,
    }
}

/// The dialog for a frame of the document, or none when there is no such frame (the dialog closes). The export feature
/// is run once for it, and only when the frame is there.
pub fn dialog(
    nodes: &[Node],
    vars: &Variables,
    choice: ModeChoice,
    id: &str,
    format: ExportFormat,
    measurer: &dyn TextMeasurer,
) -> Option<ExportDialog> {
    let frame = nodes
        .iter()
        .find(|n| n.id == id && n.kind == NodeKind::Frame)?;
    let name = label(&frame.name, MAX_LABEL);
    let mut d = ExportDialog {
        frame: id.to_string(),
        name,
        format,
        html: String::new(),
        css: String::new(),
        css_file: String::new(),
        html_bytes: None,
        css_bytes: None,
        counted_bytes: None,
        limit: MAX_OUTPUT_BYTES,
        unsupported: Vec::new(),
        warnings: Vec::new(),
        assets: Vec::new(),
        error: None,
    };
    let style = match format {
        ExportFormat::Tailwind => Format::Tailwind,
        ExportFormat::Css => Format::Css,
    };
    let options = ExportOptions::new(style, measurer).modes(choice);
    match export::export_frame(nodes, vars, id, &options) {
        Ok(e) => {
            d.css_file = options.css_href;
            d.counted_bytes = Some(e.html.len() + e.css.len());
            d.html_bytes = Some(e.html.len());
            // Plain CSS has no second file: its size is 0, not unknown.
            d.css_bytes = Some(0);
            let names = |f: &[Finding]| f.iter().map(|f| finding(f, nodes)).collect();
            d.unsupported = names(&e.report.unsupported);
            d.warnings = names(&e.report.warnings);
            d.assets = e.report.assets;
            d.html = e.html;
            // Plain CSS carries its styles inline: the stylesheet is a second file of the Tailwind form alone.
            if format == ExportFormat::Tailwind {
                d.css_bytes = Some(e.css.len());
                d.css = e.css;
            }
        }
        // The dialog is opened on a frame of the document, so only the limit happens; the others keep their code.
        Err(e) => {
            if let ExportError::TooLarge { bytes, .. } = e {
                d.counted_bytes = Some(bytes);
            }
            d.error = Some(e.code().to_string());
        }
    }
    Some(d)
}

fn finding(f: &Finding, nodes: &[Node]) -> ExportFinding {
    ExportFinding {
        code: f.code.to_string(),
        node: f.node.clone(),
        node_name: f
            .node
            .as_deref()
            .and_then(|id| nodes.iter().find(|n| n.id == id))
            .map(|n| label(&n.name, MAX_LABEL)),
        message: f.message.clone(),
    }
}

/// What the open dialog was built from. While it is the same, the document, the modes, the frame and the format are the
/// same, and so is the export: nothing is built again.
#[derive(Debug, PartialEq)]
struct Key {
    version: u64,
    frame: String,
    format: ExportFormat,
    modes: BTreeMap<String, String>,
}

/// What the dialog was built from, and how often (a test seam: the count must stay as it is while no dialog is open, and
/// while what the export reads has not changed).
#[derive(Debug, Default)]
pub struct Cache {
    version: u64,
    builds: u64,
    key: Option<Key>,
    /// The variables the open Export variables dialog was built from, and how often it was built.
    vars_key: Option<Variables>,
    vars_builds: u64,
}

impl Cache {
    /// The document or the variables changed.
    pub fn changed(&mut self) {
        self.version += 1;
    }

    /// How many times the export feature was run for the dialog.
    pub fn builds(&self) -> u64 {
        self.builds
    }

    /// The dialog for `frame` was built from the document as it is now and these modes.
    pub fn built(&mut self, frame: &str, format: ExportFormat, modes: &BTreeMap<String, String>) {
        self.builds += 1;
        self.key = Some(Key {
            version: self.version,
            frame: frame.to_string(),
            format,
            modes: modes.clone(),
        });
    }

    /// The dialog is closed: nothing is remembered.
    pub fn forget(&mut self) {
        self.key = None;
    }

    /// How many times the export feature was run for the Export variables dialog.
    pub fn variables_builds(&self) -> u64 {
        self.vars_builds
    }

    /// The Export variables dialog was built from these variables.
    pub fn variables_built(&mut self, vars: &Variables) {
        self.vars_builds += 1;
        self.vars_key = Some(vars.clone());
    }

    /// The Export variables dialog is closed: nothing is remembered.
    pub fn variables_forget(&mut self) {
        self.vars_key = None;
    }

    /// Whether the open Export variables dialog was built from these variables. A node that moves does not change
    /// them, so it does not build the text again.
    pub fn variables_fresh(&self, vars: &Variables) -> bool {
        self.vars_key.as_ref() == Some(vars)
    }

    /// Whether the open dialog was built from what the export reads now.
    pub fn is_fresh(
        &self,
        frame: &str,
        format: ExportFormat,
        modes: &BTreeMap<String, String>,
    ) -> bool {
        matches!(&self.key, Some(k)
            if k.version == self.version && k.frame == frame && k.format == format && k.modes == *modes)
    }
}
