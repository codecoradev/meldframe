//! The Export dialog as plain data (D9): which frame an export acts on, and the document built for it while the
//! dialog is open. The rules that fill it are in `meldframe-app`; a front end draws it and sends the actions.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What the Export section and the dialog act on, and the dialog while it is open.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportState {
    /// The one selected node when it is a frame: the frame an export (HTML or PNG) is made of. `None` for no
    /// selection, for several nodes and for a node that is not a frame.
    pub target: Option<ExportTarget>,
    /// The dialog, `Some` only while it is open. It keeps its own frame when the selection moves on.
    pub dialog: Option<ExportDialog>,
}

/// A frame that can be exported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportTarget {
    pub id: String,
    pub name: String,
}

/// How the styles of the HTML are written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    /// Tailwind v4 utilities on each element; `css` is the input of the Tailwind compiler.
    #[default]
    Tailwind,
    /// One rule per node in an inline stylesheet.
    Css,
}

/// The open Export dialog: the document of one frame in one format, as the person sees the file now (what the
/// server has not confirmed yet, and the modes on view, are in it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportDialog {
    /// The id and the name of the frame.
    pub frame: String,
    pub name: String,
    pub format: ExportFormat,
    /// The whole HTML document, and the stylesheet that goes with the Tailwind form (empty for plain CSS, which
    /// carries its styles inline). Both are empty when `error` is set.
    pub html: String,
    pub css: String,
    /// The name the document links the stylesheet by, and so the name to save it under (Tailwind form).
    pub css_file: String,
    /// What each file is in bytes: the document, and the stylesheet that is saved beside it in the Tailwind form (0 for
    /// plain CSS, which has no second file). Both are `None` when `error` is set: there is no file, so its size is not
    /// known, and a front end must not draw it as 0 bytes (rule U9).
    #[ts(type = "number | null")]
    pub html_bytes: Option<usize>,
    #[ts(type = "number | null")]
    pub css_bytes: Option<usize>,
    /// What the limit is checked against, and the limit. The export counts what it hands back: in the plain CSS form
    /// that is the document and the stylesheet again (it is inside the document), so this can be more than what is
    /// saved. With `error` it is the size that was refused when the error is the limit (`E_EXPORT_TOO_LARGE`), and
    /// `None` for any other error.
    #[ts(type = "number | null")]
    pub counted_bytes: Option<usize>,
    #[ts(type = "number")]
    pub limit: usize,
    /// The properties the export dropped or approximated, what it warns about (layout findings, bindings that do not
    /// resolve, text a browser breaks differently) and the ids of the images the document refers to.
    pub unsupported: Vec<ExportFinding>,
    pub warnings: Vec<ExportFinding>,
    pub assets: Vec<String>,
    /// Why there is no document: `E_EXPORT_TOO_LARGE` (`docs/error-codes.md`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
}

/// One thing the export reports. `code` is stable; `message` is the explanation as the export writes it (English).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportFinding {
    pub code: String,
    /// The node it is about, with its name, or none for the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub node: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub node_name: Option<String>,
    pub message: String,
}
