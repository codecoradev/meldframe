//! Corrections: the ops that set right an undo or redo that was already sent.
//!
//! A *stale op* is an undo or redo op that is on its way (the transport has taken it) when another
//! actor's op is ordered before the edit it reverses: the values it carries were worked out before
//! that op and the server will apply them. An op on its way cannot be recalled, so the replica works
//! out what the rebuilt steps would write (`want`) and sends a *correction*: an `UpdateNode` for every
//! field where the view, which still holds the stale op, differs from it. Every plain field is
//! corrected, `image` included. A stale `Reparent`, `Reorder`, `AddNode` or `RemoveNode` is not.

use meldframe_core::normalize_positions;
use meldframe_history::FileState;
use meldframe_protocol::{Command, Node, NodePatch};
use serde_json::{Map, Value};
use std::collections::HashMap;

fn fields(n: &Node) -> Map<String, Value> {
    match serde_json::to_value(n) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

/// What `cmd` would undo in the tree `want`, as commands; `None` when it does not apply there.
pub(crate) fn inverse_in(want: &[Node], cmd: &Command) -> Option<Vec<Command>> {
    // `want` comes from the replica's own checked state, so no load check; only the migration, as before.
    let mut nodes = want.to_vec();
    normalize_positions(&mut nodes);
    let mut scratch = FileState::new();
    scratch.reset_nodes(nodes);
    scratch.apply(cmd).ok()?;
    scratch.undo()
}

/// `UpdateNode` commands that give each node of `have` the field values it holds in `want`. An image
/// that `want` does not have is a `null` patch.
pub(crate) fn field_diff(have: &[Node], want: &[Node]) -> Vec<Command> {
    let have: HashMap<&str, Map<String, Value>> =
        have.iter().map(|n| (n.id.as_str(), fields(n))).collect();
    let single = |k: &str, v: &Value| {
        let one = Value::Object(Map::from_iter([(k.to_string(), v.clone())]));
        serde_json::from_value::<NodePatch>(one).is_ok()
    };
    let mut out = Vec::new();
    for w in want {
        let Some(h) = have.get(w.id.as_str()) else {
            continue;
        };
        let mut wv = fields(w);
        wv.entry("image").or_insert(Value::Null);
        let patch: Map<String, Value> = wv
            .into_iter()
            .filter(|(k, v)| h.get(k).unwrap_or(&Value::Null) != v && single(k, v))
            .collect();
        if !patch.is_empty()
            && let Ok(patch) = serde_json::from_value::<NodePatch>(Value::Object(patch))
        {
            out.push(Command::UpdateNode {
                id: w.id.clone(),
                patch,
            });
        }
    }
    out
}

/// The correction `c` without the fields that the remote command `by` wrote: the remote op is ordered
/// before the correction, so those fields are not mine to set any more.
pub(crate) fn without_fields(c: &Command, by: &Command) -> Option<Command> {
    let (Command::UpdateNode { id, patch }, Command::UpdateNode { id: rid, patch: rp }) = (c, by)
    else {
        return Some(c.clone());
    };
    if id != rid {
        return Some(c.clone());
    }
    let (mut mine, theirs) = (as_map(patch), as_map(rp));
    mine.retain(|k, _| !theirs.contains_key(k));
    let patch = serde_json::from_value(Value::Object(mine)).ok()?;
    Some(Command::UpdateNode {
        id: id.clone(),
        patch,
    })
}

fn as_map(p: &NodePatch) -> Map<String, Value> {
    match serde_json::to_value(p) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}
