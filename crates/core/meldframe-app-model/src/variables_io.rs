//! The Export variables and Import variables dialogs as plain data (D8): the text of the export while its dialog is
//! open, and the result of the last import. The rules that fill them are in `meldframe-app`, the export in the `export`
//! feature and the import in the `variables-import` feature; a front end draws them and sends the actions.

use crate::event::VariableEntity;
use crate::export::ExportFinding;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The two dialogs. Both are view state: nothing here is written to the file (an import writes through the commands it
/// makes, as any edit does).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariablesIo {
    /// The Export variables dialog, `Some` only while it is open.
    pub export: Option<VariablesExportDialog>,
    /// What the last import came to, `Some` from the import until the dialog is closed or an import begins again.
    pub import: Option<VariablesImportResult>,
    /// Whether the Import variables dialog is open (`Action::OpenVariablesImport` and `CloseVariablesImport`).
    #[serde(default)]
    pub import_open: bool,
    /// The name of the file the Import field was filled from, made safe to show (`meldframe_text::label`) and cut to
    /// `MAX_FILE_NAME` characters. `Some("")` is a file whose name is nothing once made safe: the front end words that
    /// case. It stays while the field's text does (a close keeps both) and goes with Clear, an edit of the text and an
    /// applied import.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub import_source: Option<String>,
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
    /// The size of `css` in bytes, and the limit of an export. With `error` it is the size that was refused when the
    /// error is the limit, and `None` for any other error (nothing was built, so the size is not known: rule U9).
    #[ts(type = "number | null")]
    pub bytes: Option<usize>,
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

/// How an import ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ImportStatus {
    /// The commands were applied, as one undo step.
    Imported,
    /// The text was read and holds no variable the model can take: nothing was applied. The report says why.
    Nothing,
    /// Nothing was applied and the text was not (all) read: `code` says why.
    Refused,
}

/// What the last import came to (`Action::ImportVariables`). An import only adds: text that holds a name the file has is refused
/// whole, and every refusal applies nothing (`code`). The result, report included, is `AppState::variables_io.import`; it goes
/// when the file no longer holds the variables the import added (an undo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariablesImportResult {
    pub status: ImportStatus,
    /// Why it was refused: `empty_text`, `E_IMPORT_TOO_LARGE` (over 256 KiB), `E_IMPORT_TOO_MANY` (more than 50 variables: the browser
    /// sends one request for each op, and a longer style sheet goes through the agent tool `import_tokens`), `outbox_full` (the list
    /// does not fit the queue), or the `E_` code of the first command the
    /// file refused (`E_DUPLICATE_ID` for a name that is already there, `E_DUPLICATE_NAME`, `E_TOO_MANY_ENTRIES`...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub code: Option<String>,
    /// What the refusing command was about, the first duplicate for example.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub at: Option<ImportAt>,
    /// The size of the text in bytes, and the most an import reads.
    #[ts(type = "number")]
    pub bytes: usize,
    #[ts(type = "number")]
    pub limit: usize,
    /// The most variables the dialog imports at a time (`E_IMPORT_TOO_MANY` above it).
    #[ts(type = "number")]
    pub max_variables: usize,
    /// What the text held: the counts and the lines of the report. `None` for text that was not read (an empty text, one
    /// over the limit): its counts are not known, and a front end must not draw them as zero (rule U9).
    pub report: Option<ImportReport>,
}

/// The collection, mode or variable a refusal is about, by the name the text gave it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportAt {
    pub entity: VariableEntity,
    pub name: String,
}

/// The report of an import (ADR-0020): what it made and what it left out.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportReport {
    #[ts(type = "number")]
    pub collections: usize,
    #[ts(type = "number")]
    pub modes: usize,
    #[ts(type = "number")]
    pub variables: usize,
    #[ts(type = "number")]
    pub colors: usize,
    #[ts(type = "number")]
    pub numbers: usize,
    #[ts(type = "number")]
    pub aliases: usize,
    /// Custom properties that were not imported.
    #[ts(type = "number")]
    pub skipped: usize,
    /// At most 200 lines, notes first.
    pub entries: Vec<ImportEntry>,
    /// Lines left out of `entries` because it was full.
    #[ts(type = "number")]
    pub omitted: usize,
}

/// One line of the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportEntry {
    pub kind: ImportEntryKind,
    /// A stable reason: `converted`, `clipped`, `rem`, `renamed`, `import_skipped`, `computed_value`...
    pub code: String,
    /// The custom property, selector or at-rule it is about.
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ImportEntryKind {
    /// Imported, but not as written.
    Converted,
    /// Not imported.
    Skipped,
    /// Something to know that is neither.
    Note,
}
