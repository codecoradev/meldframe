//! The Export variables and Import variables dialogs (D8). The export is the `export` feature's text of every variable
//! of the file, built from the person's own view and kept while its dialog is open; the import is `variables-import`'s
//! reading of CSS text, applied as `Action::EditVariables` applies commands (`apply_variables`, after the same check
//! of the whole list: one undo step, the person's own edit, sent to the server as ops like any other). The export text and the plan are made by the
//! `export-dialog` and `variables-import-dialog` crates, and this decides when. Nothing is applied by a refusal.

use crate::app::App;
use crate::document::refuse;
use crate::event::Event;
use export_dialog::variables_dialog;
use meldframe_app_model::{Connection, ImportStatus, VariablesImportResult};
use meldframe_app_parts::document::code;
use meldframe_protocol::Command;
use meldframe_sync::SubmitError;
use meldframe_text::{MAX_FILE_NAME, label};
use variables_import_dialog::plan;

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

    pub(crate) fn open_variables_import(&mut self) {
        self.set_import_open(true);
    }

    /// Close the dialog; the result of the last import goes with it.
    pub(crate) fn close_variables_import(&mut self) {
        self.clear_variables_import();
        self.set_import_open(false);
    }

    /// Forget the result and leave the dialog as it is.
    pub(crate) fn clear_variables_import(&mut self) {
        self.import_ids.clear();
        self.set_import(None);
    }

    /// The field was cleared or its text replaced: the result and the name of the file it came from go.
    pub(crate) fn clear_variables_import_field(&mut self) {
        let (result, source) = (
            self.state.variables_io.import.take(),
            self.state.variables_io.import_source.take(),
        );
        self.import_ids.clear();
        if result.is_some() || source.is_some() {
            self.events.push(Event::VariablesIoChanged {});
        }
    }

    /// A file was chosen for the field: its name is made safe to show, and cut, once here (rule U9).
    pub(crate) fn choose_import_source(&mut self, name: &str) {
        self.set_import_source(Some(label(name, MAX_FILE_NAME)));
    }

    /// The result of an applied import is about what the file holds: when an undo, or someone else, takes away a variable
    /// it added, the result goes, so an open dialog does not say "Imported" about a file that no longer has it.
    pub(crate) fn refresh_variables_import(&mut self) {
        let vars = self.replica.vars();
        if !self.import_ids.is_empty()
            && self.import_ids.iter().any(|id| vars.variable(id).is_none())
        {
            self.clear_variables_import();
        }
    }

    fn set_import_open(&mut self, open: bool) {
        if self.state.variables_io.import_open != open {
            self.state.variables_io.import_open = open;
            self.events.push(Event::VariablesIoChanged {});
        }
    }

    fn set_import_source(&mut self, name: Option<String>) {
        if self.state.variables_io.import_source != name {
            self.state.variables_io.import_source = name;
            self.events.push(Event::VariablesIoChanged {});
        }
    }

    fn set_import(&mut self, result: Option<VariablesImportResult>) {
        if self.state.variables_io.import != result {
            self.state.variables_io.import = result;
            self.events.push(Event::VariablesIoChanged {});
        }
    }

    /// Read the text and apply what it holds as one undo step, or apply nothing and say why. The result is kept in
    /// `AppState::variables_io.import` either way.
    pub(crate) fn import_variables(&mut self, text: &str) -> Result<(), SubmitError> {
        let mut plan = plan(text, self.nodes(), self.vars());
        let mut done = Ok(());
        if plan.result.status == ImportStatus::Imported {
            done = self
                .check_room(&plan.commands)
                .and_then(|()| self.apply_variables(&plan.commands));
            if let Err(e) = &done {
                // The queue has no room (or, never expected after the plan's own check, the replica disagrees).
                plan.result.status = ImportStatus::Refused;
                plan.result.code = Some(code(e).to_string());
            }
        }
        self.import_ids.clear();
        if plan.result.status == ImportStatus::Imported {
            self.import_ids = plan
                .commands
                .iter()
                .filter_map(|c| match c {
                    Command::AddVariable { variable } => Some(variable.id.clone()),
                    _ => None,
                })
                .collect();
        }
        if plan.result.status == ImportStatus::Imported {
            // The field is emptied by an applied import, and with it the note about its file.
            self.set_import_source(None);
        }
        self.set_import(Some(plan.result));
        done
    }
}
