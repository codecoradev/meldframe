//! Restore (E5.3): the commands that bring the file back to an earlier state. They are new ops at the end of the
//! log, so history is never rewritten, and they carry the person's actor like any edit, so one undo takes the
//! restore back.

use crate::{LoggedOp, RevertError};
use meldframe_history::FileState;
use meldframe_protocol::{Command, Node, NodePatch};
use std::collections::{HashMap, HashSet};

/// A restore that cannot be worked out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreError {
    /// A command for node `id` does not apply or does not read back as a patch. Nothing was changed.
    DoesNotApply { id: String },
}

/// The tree as it was after op `seq`: `base` is the tree before the first op of `log`, and the ops with a
/// number above `seq` are left out.
pub fn state_at(base: &[Node], log: &[LoggedOp], seq: u64) -> Result<Vec<Node>, RevertError> {
    let mut fs = FileState::from_nodes_unchecked(base.to_vec());
    for op in log.iter().take_while(|op| op.seq <= seq) {
        fs.replay_pending(&op.command)
            .map_err(|_| RevertError::LogDoesNotReplay { seq: op.seq })?;
    }
    Ok(fs.nodes().to_vec())
}

/// The commands that turn `current` into `target`; the sibling order comes back, though a `pos` that another
/// sibling holds by then is resolved by the apply rule, so its string can differ. In the order to apply them: remove the nodes that must be
/// replaced, add what is missing (parents first), move the nodes that stayed (parents first, so no move makes a
/// cycle), remove what is left over, then set the fields that differ. A node whose kind differs is replaced with
/// its whole subtree. Checked against `current` before it is returned, so the list applies whole.
pub fn restore_commands(target: &[Node], current: &[Node]) -> Result<Vec<Command>, RestoreError> {
    let want: HashMap<&str, &Node> = target.iter().map(|n| (n.id.as_str(), n)).collect();
    let have: HashMap<&str, &Node> = current.iter().map(|n| (n.id.as_str(), n)).collect();
    let replaced: HashSet<&str> = current
        .iter()
        .filter(|n| want.get(n.id.as_str()).is_some_and(|t| t.kind != n.kind))
        .map(|n| n.id.as_str())
        .collect();
    // Gone: not in the target, replaced, or under a replaced node.
    let gone: HashSet<&str> = current
        .iter()
        .filter(|n| {
            !want.contains_key(n.id.as_str())
                || replaced.contains(n.id.as_str())
                || under_any(&have, &n.id, &replaced)
        })
        .map(|n| n.id.as_str())
        .collect();
    let mut cmds = Vec::new();
    // 1. Remove the roots of what is replaced, so their ids are free again.
    for id in &replaced {
        if !under_any(&have, id, &replaced_above(&have, id, &replaced)) {
            cmds.push(Command::RemoveNode { id: id.to_string() });
        }
    }
    let removed_early: HashSet<&str> = current
        .iter()
        .filter(|n| under_any(&have, &n.id, &replaced) || replaced.contains(n.id.as_str()))
        .map(|n| n.id.as_str())
        .collect();
    // 2. Add what the file lacks, parents first.
    let mut order: Vec<&Node> = target.iter().collect();
    order.sort_by(|a, b| {
        (depth(&want, &a.id), &a.pos, &a.id).cmp(&(depth(&want, &b.id), &b.pos, &b.id))
    });
    let keep = |id: &str| have.contains_key(id) && !gone.contains(id);
    for n in order.iter().filter(|n| !keep(&n.id)) {
        cmds.push(Command::AddNode {
            node: (*n).clone(),
            index: None,
        });
    }
    // 3. Move the nodes that stayed, parents first.
    for t in order.iter().filter(|n| keep(&n.id)) {
        let c = have[t.id.as_str()];
        if c.parent_id != t.parent_id || c.pos != t.pos {
            cmds.push(Command::Reparent {
                id: t.id.clone(),
                parent_id: t.parent_id.clone(),
                pos: (!t.pos.is_empty()).then(|| t.pos.clone()),
            });
        }
    }
    // 4. Remove what is left over: the top of each leftover subtree.
    for n in current {
        let id = n.id.as_str();
        let leftover = gone.contains(id) && !removed_early.contains(id);
        let parent_leftover = n
            .parent_id
            .as_deref()
            .is_some_and(|p| gone.contains(p) && !removed_early.contains(p));
        if leftover && !parent_leftover {
            cmds.push(Command::RemoveNode { id: id.to_string() });
        }
    }
    // 5. Set the fields that differ.
    for t in order.iter().filter(|n| keep(&n.id)) {
        if let Some(patch) = field_patch(have[t.id.as_str()], t)? {
            cmds.push(Command::UpdateNode {
                id: t.id.clone(),
                patch,
            });
        }
    }
    let mut check = FileState::from_nodes_unchecked(current.to_vec());
    for c in &cmds {
        check
            .replay_pending(c)
            .map_err(|_| RestoreError::DoesNotApply { id: command_id(c) })?;
    }
    Ok(cmds)
}

fn command_id(c: &Command) -> String {
    match c {
        Command::UpdateNode { id, .. }
        | Command::RemoveNode { id }
        | Command::Reparent { id, .. }
        | Command::Reorder { id, .. } => id.clone(),
        Command::AddNode { node, .. } => node.id.clone(),
    }
}

/// How far down a node is, from the root of its tree (a cycle stops the count).
fn depth(nodes: &HashMap<&str, &Node>, id: &str) -> usize {
    let mut d = 0;
    let mut at = nodes.get(id).and_then(|n| n.parent_id.as_deref());
    while let Some(p) = at {
        d += 1;
        if d > nodes.len() {
            break;
        }
        at = nodes.get(p).and_then(|n| n.parent_id.as_deref());
    }
    d
}

/// True when a strict ancestor of `id` is in `set`.
fn under_any(nodes: &HashMap<&str, &Node>, id: &str, set: &HashSet<&str>) -> bool {
    let mut steps = 0;
    let mut at = nodes.get(id).and_then(|n| n.parent_id.as_deref());
    while let Some(p) = at {
        if set.contains(p) {
            return true;
        }
        steps += 1;
        if steps > nodes.len() {
            break;
        }
        at = nodes.get(p).and_then(|n| n.parent_id.as_deref());
    }
    false
}

/// The replaced nodes that are strict ancestors of `id`.
fn replaced_above<'a>(
    nodes: &HashMap<&str, &Node>,
    id: &str,
    replaced: &HashSet<&'a str>,
) -> HashSet<&'a str> {
    replaced
        .iter()
        .copied()
        .filter(|r| {
            let mut steps = 0;
            let mut at = nodes.get(id).and_then(|n| n.parent_id.as_deref());
            while let Some(p) = at {
                if p == *r {
                    return true;
                }
                steps += 1;
                if steps > nodes.len() {
                    break;
                }
                at = nodes.get(p).and_then(|n| n.parent_id.as_deref());
            }
            false
        })
        .collect()
}

/// The patch that turns node `have` into `want` (same id and kind): every field that differs, a field that
/// `want` has no value for set to `null`. Read from the JSON form, so a field added to the node is covered.
fn field_patch(have: &Node, want: &Node) -> Result<Option<NodePatch>, RestoreError> {
    let bad = || RestoreError::DoesNotApply {
        id: want.id.clone(),
    };
    let (Ok(serde_json::Value::Object(h)), Ok(serde_json::Value::Object(w))) =
        (serde_json::to_value(have), serde_json::to_value(want))
    else {
        return Err(bad());
    };
    let mut patch = serde_json::Map::new();
    for key in h.keys().chain(w.keys()) {
        if matches!(key.as_str(), "id" | "type" | "parentId" | "pos") {
            continue;
        }
        if h.get(key) != w.get(key) {
            patch.insert(key.clone(), w.get(key).cloned().unwrap_or_default());
        }
    }
    if patch.is_empty() {
        return Ok(None);
    }
    serde_json::from_value(serde_json::Value::Object(patch))
        .map(Some)
        .map_err(|_| bad())
}
