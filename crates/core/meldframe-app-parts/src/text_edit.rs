//! The rules of the text being edited that read no state of the app: which node an edit can open on, and what committing
//! the field comes to. The state (`AppState::text_edit`) and the events are `meldframe-app`'s (`text_edit.rs`).

use crate::document::edit_text;
use meldframe_app_model::TextEdit;
use meldframe_protocol::{Command, Node, NodeKind};

/// The node when it is a text node of the document: the only kind that has a text to edit.
pub fn text_node<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
    nodes
        .iter()
        .find(|n| n.id == id && n.kind == NodeKind::Text)
}

/// The one selected node when it is a text node (what Enter edits): none for no selection, several, or a node of another kind.
pub fn selected_text<'a>(nodes: &'a [Node], selection: &[String]) -> Option<&'a Node> {
    match selection {
        [id] => text_node(nodes, id),
        _ => None,
    }
}

/// What committing the field does: one command for a text that differs from the text the edit began with, nothing for an
/// equal one (an unchanged text makes no undo step).
pub fn commit_command(edit: &TextEdit, text: String) -> Option<Command> {
    (text != edit.text).then(|| edit_text(&edit.id, text))
}
