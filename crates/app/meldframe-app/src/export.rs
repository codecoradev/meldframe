//! The Export dialog (D9): the frame an export acts on, and the HTML built for it while the dialog is open. The export
//! is the `export` feature's, run on the person's own view of the file (pending ops included, the modes on view). Text
//! is measured by the layout core's stub, as the MCP tool `export_frame` does: the app layer has no browser. View
//! state: nothing is written to the file and the undo history does not change.

use crate::app::App;
use crate::document::refuse;
use crate::event::Event;
use export::{Export, ExportError, ExportOptions, Finding, Format, MAX_OUTPUT_BYTES};
use meldframe_app_model::{Connection, ExportDialog, ExportFinding, ExportFormat, ExportTarget};
use meldframe_layout::StubMeasurer;
use meldframe_protocol::{Node, NodeKind};
use std::collections::BTreeMap;

/// What the open dialog was built from. While it is the same, the document, the modes, the frame and the format are the
/// same, and so is the export: nothing is built again.
#[derive(Debug, PartialEq)]
struct Key {
    version: u64,
    frame: String,
    format: ExportFormat,
    modes: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
pub(crate) struct ExportCache {
    /// Counted up by `App::changed`, the one place that says the document or the variables changed.
    pub(crate) version: u64,
    builds: u64,
    key: Option<Key>,
}

impl App {
    /// How many times the export feature was run for the dialog (a test seam: it must stay as it is while no dialog is
    /// open, and while what the export reads has not changed). Counted where the feature is called.
    #[doc(hidden)]
    pub fn export_builds(&self) -> u64 {
        self.export_cache.builds
    }

    pub(crate) fn open_export(&mut self) {
        // A halted app may hold a view that is out of date: it exports nothing until a new snapshot is loaded.
        let code = if self.state.connection == Connection::Halted {
            Some("out_of_sync")
        } else if self.state.export.target.is_none() {
            Some("not_exportable")
        } else {
            None
        };
        if let Some(code) = code {
            let _ = refuse(&mut self.events, code);
            return;
        }
        let id = self.state.export.target.as_ref().map(|t| t.id.clone());
        if let Some(dialog) = id.and_then(|id| self.build_dialog(&id, ExportFormat::default())) {
            self.set_dialog(Some(dialog));
        }
    }

    pub(crate) fn close_export(&mut self) {
        self.export_cache.key = None;
        self.set_dialog(None);
    }

    pub(crate) fn set_export_format(&mut self, format: ExportFormat) {
        let Some(d) = &self.state.export.dialog else {
            return;
        };
        if d.format != format {
            let id = d.frame.clone();
            let dialog = self.build_dialog(&id, format);
            self.set_dialog(dialog);
        }
    }

    /// Bring the target and the open dialog in line with the selection and the document. The dialog closes when its
    /// frame is gone or the app halts; nothing is built while it is closed, and nothing again while what the export reads
    /// is as it was.
    pub(crate) fn refresh_export(&mut self) {
        let target = match self.state.selection.as_slice() {
            [id] => self
                .nodes()
                .iter()
                .find(|n| n.id == *id && n.kind == NodeKind::Frame)
                .map(|n| ExportTarget {
                    id: n.id.clone(),
                    name: n.name.clone(),
                }),
            _ => None,
        };
        if self.state.export.target != target {
            self.state.export.target = target;
            self.events.push(Event::ExportChanged {});
        }
        let Some(d) = &self.state.export.dialog else {
            return;
        };
        let (id, format) = (d.frame.clone(), d.format);
        if self.state.connection == Connection::Halted {
            self.close_export();
            return;
        }
        let fresh = matches!(&self.export_cache.key, Some(k)
            if k.version == self.export_cache.version && k.frame == id && k.format == format && k.modes == self.state.modes);
        if fresh {
            return;
        }
        let next = self.build_dialog(&id, format);
        self.set_dialog(next);
    }

    /// Set the dialog, and say so when it is not what it was: the web reads the state again after an event.
    fn set_dialog(&mut self, dialog: Option<ExportDialog>) {
        if self.state.export.dialog != dialog {
            self.state.export.dialog = dialog;
            self.events.push(Event::ExportChanged {});
        }
    }

    /// The dialog for a frame of the document, or none when there is no such frame (the dialog closes).
    fn build_dialog(&mut self, id: &str, format: ExportFormat) -> Option<ExportDialog> {
        let name = self
            .nodes()
            .iter()
            .find(|n| n.id == id && n.kind == NodeKind::Frame)?
            .name
            .clone();
        self.export_cache.key = Some(Key {
            version: self.export_cache.version,
            frame: id.to_string(),
            format,
            modes: self.state.modes.clone(),
        });
        let mut d = ExportDialog {
            frame: id.to_string(),
            name,
            format,
            html: String::new(),
            css: String::new(),
            css_file: String::new(),
            html_bytes: 0,
            css_bytes: 0,
            counted_bytes: 0,
            limit: MAX_OUTPUT_BYTES,
            unsupported: Vec::new(),
            warnings: Vec::new(),
            assets: Vec::new(),
            error: None,
        };
        match self.run_export(id, format) {
            Ok((e, css_file)) => {
                d.css_file = css_file;
                d.counted_bytes = e.html.len() + e.css.len();
                d.html_bytes = e.html.len();
                let names = |f: &[Finding]| f.iter().map(|f| finding(f, self.nodes())).collect();
                d.unsupported = names(&e.report.unsupported);
                d.warnings = names(&e.report.warnings);
                d.assets = e.report.assets;
                d.html = e.html;
                // Plain CSS carries its styles inline: the stylesheet is a second file of the Tailwind form alone.
                if format == ExportFormat::Tailwind {
                    d.css_bytes = e.css.len();
                    d.css = e.css;
                }
            }
            // The dialog is opened on a frame of the document, so only the limit happens; the others keep their code.
            Err(e) => {
                if let ExportError::TooLarge { bytes, .. } = e {
                    d.counted_bytes = bytes;
                }
                d.error = Some(e.code().to_string());
            }
        }
        Some(d)
    }

    /// The one place the export feature is run for the dialog, and where its runs are counted.
    fn run_export(
        &mut self,
        id: &str,
        format: ExportFormat,
    ) -> Result<(Export, String), ExportError> {
        self.export_cache.builds += 1;
        let style = match format {
            ExportFormat::Tailwind => Format::Tailwind,
            ExportFormat::Css => Format::Css,
        };
        let options = ExportOptions::new(style, &StubMeasurer).modes(self.choice());
        let e = export::export_frame(self.nodes(), self.vars(), id, &options)?;
        Ok((e, options.css_href))
    }
}

fn finding(f: &Finding, nodes: &[Node]) -> ExportFinding {
    ExportFinding {
        code: f.code.to_string(),
        node: f.node.clone(),
        node_name: f
            .node
            .as_deref()
            .and_then(|id| nodes.iter().find(|n| n.id == id))
            .map(|n| n.name.clone()),
        message: f.message.clone(),
    }
}
