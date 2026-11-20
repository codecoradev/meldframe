//! The Export dialog (D9): the frame an export acts on, and the HTML built for it while the dialog is open. The export
//! is the `export` feature's, run on the person's own view of the file (pending ops included, the modes on view); the
//! target and the dialog are made by the `export-dialog` crate, and this decides when they are made. View state:
//! nothing is written to the file and the undo history does not change.

use crate::app::App;
use crate::document::refuse;
use crate::event::Event;
use export_dialog::{dialog, target};
use meldframe_app_model::{Connection, ExportDialog, ExportFormat};

impl App {
    /// How many times the export feature was run for the dialog (a test seam: it must stay as it is while no dialog is
    /// open, and while what the export reads has not changed). Counted where the feature is called.
    #[doc(hidden)]
    pub fn export_builds(&self) -> u64 {
        self.export_cache.builds()
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
        self.export_cache.forget();
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
        let target = target(&self.state.selection, self.nodes());
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
        if self.export_cache.is_fresh(&id, format, &self.state.modes) {
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

    /// The dialog for a frame of the document, or none when there is no such frame (the dialog closes). The export
    /// feature is run, and counted, only when the frame is there.
    fn build_dialog(&mut self, id: &str, format: ExportFormat) -> Option<ExportDialog> {
        let d = dialog(self.nodes(), self.vars(), self.choice(), id, format)?;
        self.export_cache.built(id, format, &self.state.modes);
        Some(d)
    }
}
