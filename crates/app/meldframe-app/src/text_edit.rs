//! The text being edited as state of the app (audit finding T12, issue 324): beginning, committing and cancelling are here, and
//! the field keeps its keystrokes, its caret and the composition of an input method (it reports the text once, on a commit).
//! The rules that read no state are `meldframe_app_parts::text_edit`; the kind of the node, the text an edit began with and
//! when an unchanged text makes no step are the rules the page kept in TypeScript before this.

use crate::app::App;
use crate::event::Event;
use meldframe_app_model::{TextEdit, TextEditOp};
use meldframe_app_parts::text_edit::{commit_command, selected_text, text_node};
use meldframe_sync::SubmitError;

impl App {
    /// An op that changes the view only: beginning an edit and cancelling one.
    pub(crate) fn text_edit_view(&mut self, op: TextEditOp) {
        match op {
            TextEditOp::Begin { id } => self.begin_text_edit(id),
            TextEditOp::BeginSelected {} => {
                if let Some(id) =
                    selected_text(self.replica.nodes(), &self.state.selection).map(|n| n.id.clone())
                {
                    self.begin_text_edit(id);
                }
            }
            TextEditOp::Cancel {} => self.state.text_edit = None,
            // The commit writes to the file: it is the document's.
            TextEditOp::Commit { .. } => {}
        }
    }

    /// Select the node, as a click does, and open the edit on it with the text it has now. A node that is not there or is
    /// not a text node does nothing and raises nothing (the page did nothing for a double click on a frame, and a refusal
    /// is a notice on screen). An edit open on another node is dropped: the field commits before this when it loses focus.
    fn begin_text_edit(&mut self, id: String) {
        let Some(text) = text_node(self.replica.nodes(), &id).map(|n| n.text.clone()) else {
            return;
        };
        self.select(vec![id.clone()]);
        self.state.text_edit = Some(TextEdit { id, text });
    }

    /// The field commits: the edit closes, and a text that differs from the one the edit began with is one change.
    pub(crate) fn text_edit_commit(&mut self, id: &str, text: String) -> Result<(), SubmitError> {
        if self.state.text_edit.as_ref().is_none_or(|e| e.id != id) {
            return Ok(());
        }
        let Some(edit) = self.state.text_edit.take() else {
            return Ok(());
        };
        match commit_command(&edit, text) {
            Some(command) => self.replica.apply(&command),
            None => Ok(()),
        }
    }

    /// While the app is halted nothing is written: the edit closes and the text is dropped.
    pub(crate) fn drop_text_edit(&mut self) {
        self.state.text_edit = None;
    }

    /// An edit belongs to a text node that is still in the document: when it is gone (another actor, or the person, removed
    /// it, or a reload brought a file without it) the edit closes with nothing to commit. Whatever else happens to the node
    /// meanwhile (a new text from another actor) leaves it open on the text it began with.
    pub(crate) fn settle_text_edit(&mut self) {
        let Some(edit) = &self.state.text_edit else {
            return;
        };
        if text_node(self.replica.nodes(), &edit.id).is_none() {
            self.state.text_edit = None;
        }
    }

    /// The event that says `AppState::text_edit` changed, once per dispatch.
    pub(crate) fn text_edit_changed(&mut self, before: &Option<TextEdit>) {
        if self.state.text_edit != *before {
            self.events.push(Event::TextEditChanged {});
        }
    }
}
