//! The text being edited (audit finding T12, issue 324, decision 43 clause Q6): which text node is open for editing and the text
//! it had when the field opened. Beginning, committing and cancelling are rules of the app layer, so a second front end (a
//! headless one among them) opens, commits and cancels an edit without writing them again. What stays in the field, and so
//! never crosses this boundary: the keystrokes, the caret, the selection inside the text and the composition of an input
//! method. The field reports the text it holds once, when it commits.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The edit that is open (`AppState::text_edit`): a text node, and the text it had when the field opened. An unchanged
/// text makes no step, and the comparison is with this text, not with what the node holds when the field commits (an edit
/// of another actor in between does not move it). The text is the node's own, unchanged: the field has to show exactly
/// what the file holds, so it is not cleaned as a name is (rule U9 is for text that is only read).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextEdit {
    /// The text node being edited. It is a text node of the document for as long as the edit is open: an edit whose node is
    /// gone (another actor removed it) is closed by the app, with nothing to commit and a `Refused` `not_found` event.
    pub id: String,
    /// The text of the node when the edit began.
    pub text: String,
}

/// What a person does with the text being edited (`Action::TextEdit`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TextEditOp {
    /// Open an edit on a text node (a double click on it): select it, as a click does, and record its text. An edit that is
    /// open on another node is dropped with nothing committed (the field commits before this, when it loses focus). A node that
    /// is not there, or is not a text node, changes nothing and raises nothing: a canvas reports a double click on any node, and
    /// a refusal is a notice on screen.
    Begin { id: String },
    /// Open an edit on the selected node when the selection is exactly one text node (Enter): anything else does nothing
    /// and raises nothing, so a key that starts no edit is the front end's to hand on.
    BeginSelected {},
    /// The field of node `id` commits with the text it holds: the edit closes, and a text that differs from the one the edit
    /// began with is written to the node as one change and one undo step (`EditText`); an equal text makes none. Nothing when no
    /// edit is open (a blur that follows a commit or a cancel) and nothing when the open edit is on another node (a field that
    /// is being replaced reports late: its text is not the other node's). A text the file refuses (it is too long) is refused
    /// as `EditText` is, and the edit is closed all the same. While the app is halted the edit closes and the text is dropped.
    Commit { id: String, text: String },
    /// The field cancels (Escape): the edit closes and the file is not touched.
    Cancel {},
}

impl TextEditOp {
    /// Whether the op changes the view only: nothing is written to the file (all but `Commit`).
    pub fn is_view(&self) -> bool {
        !matches!(self, TextEditOp::Commit { .. })
    }
}
