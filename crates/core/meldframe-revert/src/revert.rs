//! Revert of a batch (E5.2, ADR-0005): the commands that restore what an actor's batch changed, field by
//! field, and only where that actor is still the last writer.
//!
//! The input is a tree and the log after it: `before` is the file as it was just before the first op of
//! `log`, and `log` runs from there (the batch and whatever came after it) to the newest op. Nothing is read
//! from a store or a clock, so the client, the server and a test all give the same answer.
//!
//! The rules:
//!
//! - A field of a node (`fill`, `x`, ...) goes back to its value before the batch only when the newest op that
//!   wrote it is an op of the batch. A write by the person, by another agent or by a later batch of the same
//!   agent keeps its value; the field is listed in [`Revert::skipped`].
//! - A node's place (parent and order) is one unit: a reparent or reorder of the batch is taken back only when
//!   no later op moved the node.
//! - A node the batch added is removed when its subtree holds only nodes the batch added and no op of anyone
//!   else touched them since. A node the batch removed comes back with its subtree.
//! - The commands apply in order, newest op of the batch first, each checked against the tree the ones before it
//!   leave, so the list applies whole. A command that no longer applies (the node is gone) is dropped and listed.

use meldframe_history::FileState;
use meldframe_protocol::{Command, Node, NodePatch};
use std::collections::{HashMap, HashSet};

/// One op of the log: what the server numbered, with the actor and the idempotency key it carried.
#[derive(Debug, Clone)]
pub struct LoggedOp {
    pub seq: u64,
    pub actor: String,
    pub key: String,
    pub command: Command,
}

/// A batch: the ops of `actor` whose keys are `{key_prefix}:{n}` (the keys of one `apply_commands` call).
#[derive(Debug, Clone, Copy)]
pub struct Batch<'a> {
    pub actor: &'a str,
    pub key_prefix: &'a str,
}

impl Batch<'_> {
    fn has(&self, op: &LoggedOp) -> bool {
        op.actor == self.actor && batch_index(&op.key).is_some_and(|(p, _)| p == self.key_prefix)
    }
}

/// Splits an op key `{prefix}:{n}` (the key of command `n` of one `apply_commands` call) into the prefix and `n`;
/// `None` for any other key, such as the one-op keys of the editor.
pub fn batch_index(key: &str) -> Option<(&str, u64)> {
    let (prefix, n) = key.rsplit_once(':')?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((prefix, n.parse().ok()?))
}

/// Why a part of the batch was left as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// Someone else wrote it (or touched the node) after the batch.
    ChangedSince,
    /// The command no longer applies to the file as it is (the node is gone, say).
    NotApplicable,
}

/// A part of the batch that was not reverted. `what` is a field name (`fill`), `place` for the parent and order
/// of the node, `node` for the node as a whole, or `variable` for a variable command (`node` then holds the id of
/// the variable or collection; a revert does not take those back).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub node: String,
    pub what: String,
    pub reason: SkipReason,
}

/// The result: commands to apply in order as one change, and what was left alone.
#[derive(Debug, Clone, Default)]
pub struct Revert {
    pub commands: Vec<Command>,
    pub skipped: Vec<Skipped>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevertError {
    /// No op of the log belongs to the batch.
    NoSuchBatch,
    /// Op `seq` does not apply to the tree before it: `before` and `log` do not fit.
    LogDoesNotReplay { seq: u64 },
}

const PLACE: &str = "place";

/// The commands that revert `batch` out of `log`; see the module doc.
pub fn revert_batch(
    before: &[Node],
    log: &[LoggedOp],
    batch: Batch<'_>,
) -> Result<Revert, RevertError> {
    if !log.iter().any(|op| batch.has(op)) {
        return Err(RevertError::NoSuchBatch);
    }
    let mut fs = FileState::from_nodes_unchecked(before.to_vec());
    let mut inverses: Vec<Vec<Command>> = Vec::with_capacity(log.len());
    for op in log {
        let inverse = fs
            .replay_pending(&op.command)
            .map_err(|_| RevertError::LogDoesNotReplay { seq: op.seq })?;
        inverses.push(inverse);
    }
    // For each (node, field): is the newest op that wrote it an op of the batch?
    let mut mine: HashMap<(String, String), bool> = HashMap::new();
    for op in log {
        for key in written(&op.command) {
            mine.insert(key, batch.has(op));
        }
    }
    let mut run = Run {
        scratch: fs,
        out: Revert::default(),
        seen: HashSet::new(),
    };
    for (i, op) in log.iter().enumerate().rev().filter(|(_, op)| batch.has(op)) {
        let inverse = &inverses[i];
        match &op.command {
            Command::UpdateNode { id, .. } => {
                let Some(Command::UpdateNode { patch, .. }) = inverse.first() else {
                    continue;
                };
                let (kept, dropped) = keep_fields(id, patch, &mine);
                for field in dropped {
                    run.skip(id, &field, SkipReason::ChangedSince);
                }
                if let Some(patch) = kept {
                    let cmd = Command::UpdateNode {
                        id: id.clone(),
                        patch,
                    };
                    run.push(cmd, id, "node");
                }
            }
            Command::Reparent { id, .. } | Command::Reorder { id, .. } => {
                if mine.get(&(id.clone(), PLACE.into())) != Some(&true) {
                    run.skip(id, PLACE, SkipReason::ChangedSince);
                } else if let Some(cmd) = inverse.first() {
                    run.push(cmd.clone(), id, PLACE);
                }
            }
            Command::AddNode { node, .. } => {
                let id = &node.id;
                if run.added_subtree_is_clean(id, log, op.seq, batch) {
                    run.push(Command::RemoveNode { id: id.clone() }, id, "node");
                } else if run.scratch.node(id).is_some() {
                    run.skip(id, "node", SkipReason::ChangedSince);
                } else {
                    // Someone else removed it since: there is nothing left to take back.
                    run.skip(id, "node", SkipReason::NotApplicable);
                }
            }
            Command::RemoveNode { id } => {
                for cmd in inverse {
                    run.push(cmd.clone(), id, "node");
                }
            }
            // A variable command is not taken back by a revert (ADR-0017, known limit): it is listed, so that
            // `skipped` does not report a full revert that is not one.
            other => {
                if let Some(id) = variable_target(other) {
                    run.skip(&id, "variable", SkipReason::NotApplicable);
                }
            }
        }
    }
    Ok(run.out)
}

/// The list being built, with the tree it applies to (the file as it is now, then each command of the list).
struct Run {
    scratch: FileState,
    out: Revert,
    seen: HashSet<(String, String)>,
}

impl Run {
    fn skip(&mut self, node: &str, what: &str, reason: SkipReason) {
        if self.seen.insert((node.to_string(), what.to_string())) {
            self.out.skipped.push(Skipped {
                node: node.to_string(),
                what: what.to_string(),
                reason,
            });
        }
    }

    /// Keep `cmd` when it applies to the tree as the list leaves it; otherwise list it as not applicable.
    fn push(&mut self, cmd: Command, node: &str, what: &str) {
        if self.scratch.replay_pending(&cmd).is_ok() {
            self.out.commands.push(cmd);
        } else {
            self.skip(node, what, SkipReason::NotApplicable);
        }
    }

    /// True when node `id`, added by the batch at `added_at`, can go: everything under it now was added by the
    /// batch, and no op outside the batch after that named any of those nodes (as the node, or as the parent of
    /// an added or moved one).
    fn added_subtree_is_clean(
        &self,
        id: &str,
        log: &[LoggedOp],
        added_at: u64,
        batch: Batch<'_>,
    ) -> bool {
        let nodes = self.scratch.nodes();
        let under = subtree(nodes, id);
        let mine: HashSet<&str> = log
            .iter()
            .filter(|op| batch.has(op))
            .filter_map(|op| match &op.command {
                Command::AddNode { node, .. } => Some(node.id.as_str()),
                _ => None,
            })
            .collect();
        under.iter().all(|n| mine.contains(n.as_str()))
            && !log
                .iter()
                .filter(|op| op.seq > added_at && !batch.has(op))
                .any(|op| named(&op.command).iter().any(|n| under.contains(n)))
    }
}

/// The ids of `id` and everything under it.
fn subtree(nodes: &[Node], id: &str) -> HashSet<String> {
    let mut set: HashSet<String> = HashSet::from([id.to_string()]);
    let mut grew = true;
    while grew {
        grew = false;
        for n in nodes {
            if n.parent_id.as_ref().is_some_and(|p| set.contains(p)) && set.insert(n.id.clone()) {
                grew = true;
            }
        }
    }
    set
}

/// Every node id a command names: the node, and the parent it is put under.
fn named(c: &Command) -> Vec<String> {
    match c {
        Command::UpdateNode { id, .. }
        | Command::RemoveNode { id }
        | Command::Reorder { id, .. } => {
            vec![id.clone()]
        }
        Command::AddNode { node, .. } => std::iter::once(node.id.clone())
            .chain(node.parent_id.clone())
            .collect(),
        Command::Reparent { id, parent_id, .. } => std::iter::once(id.clone())
            .chain(parent_id.clone())
            .collect(),
        // Revert covers node commands only (variable commands are a known limit, see ADR-0017).
        _ => Vec::new(),
    }
}

/// The variable or collection a variable command is about (`None` for a node command).
fn variable_target(c: &Command) -> Option<String> {
    match c {
        Command::AddCollection { collection } => Some(collection.id.clone()),
        Command::RemoveCollection { id }
        | Command::RenameCollection { id, .. }
        | Command::RemoveVariable { id }
        | Command::RenameVariable { id, .. }
        | Command::SetVariableValue { id, .. } => Some(id.clone()),
        Command::AddMode { collection, .. }
        | Command::RemoveMode { collection, .. }
        | Command::RenameMode { collection, .. } => Some(collection.clone()),
        Command::AddVariable { variable } => Some(variable.id.clone()),
        _ => None,
    }
}

/// The `(node, field)` pairs a command writes. A reparent and a reorder write the place of the node; an add
/// and a remove write no field (they are handled as a whole).
fn written(c: &Command) -> Vec<(String, String)> {
    match c {
        Command::UpdateNode { id, patch } => patch_keys(patch)
            .into_iter()
            .map(|k| (id.clone(), k))
            .collect(),
        Command::Reparent { id, .. } | Command::Reorder { id, .. } => {
            vec![(id.clone(), PLACE.to_string())]
        }
        _ => Vec::new(),
    }
}

/// The names of the fields a patch sets, as they are on the wire. Read from the JSON form, so a field added to
/// `NodePatch` is covered without a change here.
fn patch_keys(patch: &NodePatch) -> Vec<String> {
    match serde_json::to_value(patch) {
        Ok(serde_json::Value::Object(m)) => m.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

/// Split an inverse patch: the fields whose newest writer is in the batch, and the names of the others.
fn keep_fields(
    id: &str,
    inverse: &NodePatch,
    mine: &HashMap<(String, String), bool>,
) -> (Option<NodePatch>, Vec<String>) {
    let Ok(serde_json::Value::Object(fields)) = serde_json::to_value(inverse) else {
        return (None, Vec::new());
    };
    let (kept, dropped): (Vec<_>, Vec<_>) = fields
        .into_iter()
        .partition(|(k, _)| mine.get(&(id.to_string(), k.clone())) == Some(&true));
    let dropped = dropped.into_iter().map(|(k, _)| k).collect();
    if kept.is_empty() {
        return (None, dropped);
    }
    let patch = serde_json::from_value(serde_json::Value::Object(kept.into_iter().collect())).ok();
    (patch, dropped)
}
