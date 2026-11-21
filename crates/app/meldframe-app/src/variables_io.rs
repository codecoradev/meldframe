//! The Export variables dialog (D8): the `export` feature's text of every variable of the file, built from the person's
//! own view and kept while the dialog is open. The text is made by the `export-dialog` crate and this decides when.
//! View state: nothing is written to the file and the undo history does not change.

use crate::app::App;
use crate::document::refuse;
use crate::event::Event;
use export_dialog::variables_dialog;
use meldframe_app_model::Connection;

impl App {
    /// How many times the export feature was run for the Export variables dialog (a test seam: it must stay as it is
    /// while no dialog is open and while the variables have not changed).
    #[doc(hidden)]
    pub fn variables_export_builds(&self) -> u64 {
        self.export_cache.variables_builds()
    }

    pub(crate) fn open_variables_export(&mut self) {
        // A halted app may hold a view that is out of date: it exports nothing until a new snapshot is loaded.
        if self.state.connection == Connection::Halted {
            let _ = refuse(&mut self.events, "out_of_sync");
            return;
        }
        self.build_variables_export();
    }

    pub(crate) fn close_variables_export(&mut self) {
        self.export_cache.variables_forget();
        self.set_variables_export(None);
    }

    /// Bring the open dialog in line with the variables: it closes when the app halts, is built again when the
    /// variables are not what it was built from, and is left alone otherwise (a node that moves changes nothing).
    pub(crate) fn refresh_variables_export(&mut self) {
        if self.state.variables_io.export.is_none() {
            return;
        }
        if self.state.connection == Connection::Halted {
            self.close_variables_export();
        } else if !self.export_cache.variables_fresh(self.vars()) {
            self.build_variables_export();
        }
    }

    fn build_variables_export(&mut self) {
        let dialog = variables_dialog(self.vars());
        self.export_cache.variables_built(self.replica.vars());
        self.set_variables_export(Some(dialog));
    }

    fn set_variables_export(&mut self, dialog: Option<meldframe_app_model::VariablesExportDialog>) {
        if self.state.variables_io.export != dialog {
            self.state.variables_io.export = dialog;
            self.events.push(Event::VariablesIoChanged {});
        }
    }
}
