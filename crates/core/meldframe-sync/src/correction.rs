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
    // Nothing is left of the correction: it owes no field any more.
    if mine.is_empty() {
        return None;
    }
    let patch = serde_json::from_value(Value::Object(mine)).ok()?;
    Some(Command::UpdateNode {
        id: id.clone(),
        patch,
    })
}

/// Whether the correction `c` sets only fields of the same node that `of` sets.
pub(crate) fn within(c: &Command, of: &Command) -> bool {
    let (Command::UpdateNode { id, patch }, Command::UpdateNode { id: oid, patch: op }) = (c, of)
    else {
        return false;
    };
    let theirs = as_map(op);
    id == oid && as_map(patch).keys().all(|k| theirs.contains_key(k))
}

/// What is left of the refused correction `c`: the fields where `base` and the view hold the same value,
/// no pending command in `pending` writes the field, and `c` wants another value.
pub(crate) fn left_in_base(
    base: &[Node],
    view: &[Node],
    pending: &[&Command],
    c: &Command,
) -> Option<Command> {
    let Command::UpdateNode { id, patch } = c else {
        return None;
    };
    let node = |ns: &[Node]| ns.iter().find(|n| n.id == *id).map(fields);
    let (b, v) = (node(base)?, node(view)?);
    let get = |m: &Map<String, Value>, k: &str| m.get(k).cloned().unwrap_or(Value::Null);
    let written: Vec<String> = pending
        .iter()
        .filter_map(|p| match p {
            Command::UpdateNode { id: i, patch } if i == id => Some(as_map(patch)),
            _ => None,
        })
        .flat_map(|m| m.keys().cloned().collect::<Vec<_>>())
        .collect();
    let mut want = as_map(patch);
    want.retain(|k, w| !written.contains(k) && get(&b, k) == get(&v, k) && get(&v, k) != *w);
    let some = !want.is_empty();
    let patch = serde_json::from_value(Value::Object(want)).ok()?;
    some.then(|| Command::UpdateNode {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(extra: Value) -> Node {
        let mut n = json!({"id": "a", "type": "rect", "parentId": null});
        n.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::from_value(n).unwrap()
    }

    fn upd(id: &str, patch: Value) -> Command {
        Command::UpdateNode {
            id: id.into(),
            patch: serde_json::from_value(patch).unwrap(),
        }
    }

    fn keys(c: Option<Command>) -> Vec<String> {
        match c {
            Some(Command::UpdateNode { patch, .. }) => as_map(&patch).keys().cloned().collect(),
            _ => vec![],
        }
    }

    #[test]
    fn a_field_a_pending_command_writes_is_not_left() {
        let (b, v) = (
            [node(json!({"x": 0.0, "y": 0.0}))],
            [node(json!({"x": 0.0, "y": 0.0}))],
        );
        let c = upd("a", json!({"x": 5.0, "y": 6.0}));
        // The pending edit writes x to the stale value itself: base and view agree, but x is not mine.
        let pend = upd("a", json!({"x": 0.0}));
        assert_eq!(keys(left_in_base(&b, &v, &[&pend], &c)), ["y"]);
        assert_eq!(keys(left_in_base(&b, &v, &[], &c)), ["x", "y"]);
        // A pending write of another node does not count.
        let other = upd("k", json!({"x": 1.0}));
        assert_eq!(keys(left_in_base(&b, &v, &[&other], &c)).len(), 2);
    }

    #[test]
    fn a_missing_node_or_a_field_the_view_changed_leaves_nothing() {
        let (b, v) = ([node(json!({"x": 0.0}))], [node(json!({"x": 3.0}))]);
        assert!(left_in_base(&b, &v, &[], &upd("a", json!({"x": 5.0}))).is_none());
        assert!(left_in_base(&b, &b, &[], &upd("zz", json!({"x": 5.0}))).is_none());
        assert!(left_in_base(&[], &v, &[], &upd("a", json!({"x": 5.0}))).is_none());
        let same = upd("a", json!({"x": 0.0}));
        assert!(
            left_in_base(&b, &b, &[], &same).is_none(),
            "base already holds it"
        );
    }

    #[test]
    fn an_image_that_the_correction_removes_is_a_null_patch() {
        let image = json!({"image": {"assetId": "a".repeat(64)}});
        let (b, v) = ([node(image.clone())], [node(image)]);
        let c = upd("a", json!({"image": null}));
        assert_eq!(keys(left_in_base(&b, &v, &[], &c)), ["image"]);
        let bare = [node(json!({}))];
        assert!(
            left_in_base(&bare, &bare, &[], &c).is_none(),
            "no image, nothing to remove"
        );
    }

    #[test]
    fn a_correction_within_the_fields_of_another_is_recognised() {
        let c = upd("a", json!({"x": 1.0}));
        assert!(within(&c, &upd("a", json!({"x": 5.0, "y": 6.0}))));
        assert!(!within(
            &upd("a", json!({"x": 1.0, "y": 1.0})),
            &upd("a", json!({"x": 5.0}))
        ));
        assert!(!within(&c, &upd("k", json!({"x": 5.0}))));
    }
}
