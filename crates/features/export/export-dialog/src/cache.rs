//! What the open Export dialogs were built from, so that nothing is built again while it is the same.

use meldframe_app_model::ExportFormat;
use meldframe_protocol::Variables;
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
