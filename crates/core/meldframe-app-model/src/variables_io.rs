//! The Export variables dialog as plain data (D8): the text of the export while its dialog is open. The rules that fill
//! it are in `meldframe-app` and the export is the `export` feature's; a front end draws it and sends the actions.

use crate::export::ExportFinding;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The variables dialogs. View state: nothing here is written to the file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariablesIo {
    /// The Export variables dialog, `Some` only while it is open.
    pub export: Option<VariablesExportDialog>,
}

/// The open Export variables dialog: every variable of the file as CSS, as the person sees the file now (what the
/// server has not confirmed yet is in it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariablesExportDialog {
    /// One declaration per line; empty when the file has no variable, and when `error` is set.
    pub css: String,
    /// Collections that hold a variable, variables, and rules written (`:root` and one for each other mode).
    #[ts(type = "number")]
    pub collections: usize,
    #[ts(type = "number")]
    pub variables: usize,
    #[ts(type = "number")]
    pub blocks: usize,
    /// The size of `css` in bytes, and the limit of an export. With `error` it is the size that was refused.
    #[ts(type = "number")]
    pub bytes: usize,
    #[ts(type = "number")]
    pub limit: usize,
    /// What could not be written as it is stored (a value missing in a mode, a colour that is not hex, an alias to a
    /// variable that is not there). `node` is none: it is about the file.
    pub warnings: Vec<ExportFinding>,
    /// Why there is no text: `E_EXPORT_TOO_LARGE` (`docs/error-codes.md`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
}
