//! The Export dialog for a frame: the target an export acts on, and the dialog built from the export feature.

use crate::export_dialog::name::file_stem;
use export::{ExportError, ExportOptions, Finding, Format, MAX_OUTPUT_BYTES};
use meldframe_app_model::{ExportDialog, ExportFinding, ExportFormat, ExportTarget};
use meldframe_layout::TextMeasurer;
use meldframe_protocol::{Node, NodeKind, Variables};
use meldframe_text::{MAX_LABEL, label};
use meldframe_variables::ModeChoice;

/// The longest report message kept, in characters. A message is the export's own sentence with the ids of nodes in it.
pub(crate) const MAX_MESSAGE: usize = 1024;

/// The frame an export acts on: the one selected node, when it is a frame.
pub fn target(selection: &[String], nodes: &[Node]) -> Option<ExportTarget> {
    match selection {
        [id] => nodes
            .iter()
            .find(|n| n.id == *id && n.kind == NodeKind::Frame)
            .map(|n| ExportTarget {
                id: n.id.clone(),
                name: label(&n.name, MAX_LABEL),
                file_stem: file_stem(&n.name, &n.id),
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
    let stem = file_stem(&frame.name, &frame.id);
    let mut d = ExportDialog {
        frame: id.to_string(),
        name,
        format,
        html: String::new(),
        css: String::new(),
        html_file: format!("{stem}.html"),
        file_stem: stem,
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
        message: label(&f.message, MAX_MESSAGE),
    }
}
