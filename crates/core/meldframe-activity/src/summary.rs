//! What a batch's ops did, in counts and at most three names: the summary of a row, and the nodes its Show selects.

use crate::MAX_COMBINING_RUN;
use meldframe_protocol::Command;
use std::collections::HashSet;

/// The most names a summary holds, and the longest.
pub const MAX_NAMES: usize = 3;
pub const MAX_NAME_CHARS: usize = 40;
/// The most node ids kept per batch for Show and for the distinct count of changed nodes.
pub const MAX_SHOWN: usize = 200;

/// Counts of what a batch added, changed and removed (nodes; a variable command is in none of them) and the names of
/// the first nodes it named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub added: u32,
    pub changed: u32,
    pub removed: u32,
    pub names: Vec<String>,
}

/// A node name as a row may show it, cleaned as a note is: hidden characters and controls gone, no more than
/// [`MAX_COMBINING_RUN`] combining marks in a row (counted once the hidden characters are gone), trimmed, cut to
/// [`MAX_NAME_CHARS`].
fn clean_name(s: &str) -> Option<String> {
    let mut run = 0;
    let s: String = s
        .chars()
        .filter(|c| !crate::is_hidden_text(*c))
        .filter(|c| {
            run = if crate::is_combining_mark(*c) {
                run + 1
            } else {
                0
            };
            run <= MAX_COMBINING_RUN
        })
        .collect();
    let s: String = s.trim().chars().take(MAX_NAME_CHARS).collect();
    let s = s.trim_end();
    (!s.is_empty()).then(|| s.to_owned())
}

/// The running tally of one batch. The counts are exact while a batch names at most [`MAX_SHOWN`] distinct nodes of a
/// kind; past that the sets that tell one node from another are full, so each further command counts once (an upper
/// bound) and is not shown.
#[derive(Debug, Clone, Default)]
pub(crate) struct Tally {
    added: u32,
    removed: u32,
    changed: u32,
    changed_ids: HashSet<String>,
    added_ids: HashSet<String>,
    names: Vec<String>,
    /// The ids the batch named and did not remove, in order of first mention, at most [`MAX_SHOWN`].
    pub(crate) shown: Vec<String>,
}

impl Tally {
    fn name(&mut self, label: &str) {
        if self.names.len() < MAX_NAMES
            && let Some(n) = clean_name(label)
            && !self.names.contains(&n)
        {
            self.names.push(n);
        }
    }

    fn show(&mut self, id: &str) {
        if self.shown.len() < MAX_SHOWN && !self.shown.iter().any(|s| s == id) {
            self.shown.push(id.to_owned());
        }
    }

    /// Count one command. A name comes from the command itself (the node's name when it adds or renames one, its id
    /// otherwise): the reducer holds no tree. A node the batch removes is not shown, and one it added and then
    /// removed is in no count.
    pub(crate) fn add(&mut self, c: &Command) {
        match c {
            Command::AddNode { node, .. } => {
                self.added = self.added.saturating_add(1);
                if self.added_ids.len() < MAX_SHOWN {
                    self.added_ids.insert(node.id.clone());
                }
                self.name(&node.name);
                self.show(&node.id);
            }
            Command::UpdateNode { id, patch } => {
                self.changed_node(id);
                self.name(patch.name.as_deref().unwrap_or(id));
            }
            Command::Reparent { id, .. } | Command::Reorder { id, .. } => {
                self.changed_node(id);
                self.name(id);
            }
            Command::RemoveNode { id } => {
                self.shown.retain(|s| s != id);
                if self.added_ids.remove(id) {
                    self.added = self.added.saturating_sub(1);
                } else {
                    self.removed = self.removed.saturating_add(1);
                    if self.changed_ids.remove(id) {
                        self.changed = self.changed.saturating_sub(1);
                    }
                }
                self.name(id);
            }
            _ => {}
        }
    }

    fn changed_node(&mut self, id: &str) {
        if !self.added_ids.contains(id) && !self.changed_ids.contains(id) {
            self.changed = self.changed.saturating_add(1);
            if self.changed_ids.len() < MAX_SHOWN {
                self.changed_ids.insert(id.to_owned());
            }
        }
        self.show(id);
    }

    pub(crate) fn summary(&self) -> Summary {
        Summary {
            added: self.added,
            changed: self.changed,
            removed: self.removed,
            names: self.names.clone(),
        }
    }
}
