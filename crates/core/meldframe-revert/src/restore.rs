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
    /// The commands apply but do not give the target (a node whose order cannot be written, say). Nothing was changed.
    Mismatch,
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
/// its whole subtree. Every removal is of a node with nothing under it by then (leaves first), so the gate
/// tiers it as the removal of that node alone. The list is replayed on `current` before it is returned: it must
/// apply whole and give `target` (sibling order included), else it is an error and nothing is sent. A moved node
/// whose target has an empty `pos` cannot be put back in order; that is `Mismatch` when the order then differs.
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
    let removed_early: HashSet<&str> = current
        .iter()
        .filter(|n| under_any(&have, &n.id, &replaced) || replaced.contains(n.id.as_str()))
        .map(|n| n.id.as_str())
        .collect();
    // 1. Remove what is replaced, leaves first: the gate tiers a removal by the tree as it is then, so a node
    // that still has content would be a Delete that waits for approval (ADR-0010).
    for id in leaves_first(&have, &removed_early) {
        cmds.push(Command::RemoveNode { id });
    }
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
                // A target without a `pos` cannot be written back; the check at the end refuses when the order
                // then differs from the target.
                pos: (!t.pos.is_empty()).then(|| t.pos.clone()),
            });
        }
    }
    // 4. Remove what is left over, leaves first (see step 1).
    let leftover: HashSet<&str> = gone.difference(&removed_early).copied().collect();
    for id in leaves_first(&have, &leftover) {
        cmds.push(Command::RemoveNode { id });
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
    if canonical(check.nodes()) != canonical(target) {
        return Err(RestoreError::Mismatch);
    }
    Ok(cmds)
}

/// The ids of `set`, each node after everything under it (deepest first, then by id for a fixed order).
fn leaves_first(nodes: &HashMap<&str, &Node>, set: &HashSet<&str>) -> Vec<String> {
    let mut ids: Vec<&str> = set.iter().copied().collect();
    ids.sort_by(|a, b| depth(nodes, b).cmp(&depth(nodes, a)).then(a.cmp(b)));
    ids.into_iter().map(String::from).collect()
}

/// The tree with list order dropped and `pos` turned into the rank among the siblings: a `pos` that another
/// sibling holds is resolved by the apply rule, so the string can differ while the order is the same.
fn canonical(nodes: &[Node]) -> Vec<serde_json::Value> {
    let mut v: Vec<&Node> = nodes.iter().collect();
    v.sort_by(|a, b| (&a.parent_id, &a.pos, &a.id).cmp(&(&b.parent_id, &b.pos, &b.id)));
    let mut rank: HashMap<Option<&str>, usize> = HashMap::new();
    let mut out: Vec<(String, serde_json::Value)> = v
        .into_iter()
        .map(|n| {
            let r = rank.entry(n.parent_id.as_deref()).or_default();
            let mut j = serde_json::to_value(n).unwrap_or_default();
            j["pos"] = (*r).into();
            *r += 1;
            (n.id.clone(), j)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.into_iter().map(|(_, j)| j).collect()
}

fn command_id(c: &Command) -> String {
    match c {
        Command::UpdateNode { id, .. }
        | Command::RemoveNode { id }
        | Command::Reparent { id, .. }
        | Command::Reorder { id, .. } => id.clone(),
        Command::AddNode { node, .. } => node.id.clone(),
        // Restore plans node commands only; a variable command is never planned here.
        _ => String::new(),
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
