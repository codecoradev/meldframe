//! The selection as a list of ids: what a toggle makes of it, and which ids the document still has.

use meldframe_protocol::Node;

/// The selection after a toggle of `id`: out when it was in, at the end when it was not.
pub fn toggled(selection: &[String], id: String) -> Vec<String> {
    let mut ids = selection.to_vec();
    match ids.iter().position(|current| *current == id) {
        Some(at) => {
            ids.remove(at);
        }
        None => ids.push(id),
    }
    ids
}

/// The ids the document has, each once, in the order given.
pub fn known(ids: Vec<String>, nodes: &[Node]) -> Vec<String> {
    let mut keep: Vec<String> = Vec::with_capacity(ids.len());
    for id in ids {
        if !keep.contains(&id) && nodes.iter().any(|n| n.id == id) {
            keep.push(id);
        }
    }
    keep
}
