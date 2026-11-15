//! Another actor's command: it is applied without a history step, and my history is rewritten against
//! it so that undo and redo never overwrite what the other actor did after me.
//!
//! The rules, per command in my history (`list` is the commands of one step, in apply order):
//! - `UpdateNode`: fields the remote command also wrote are dropped; a step with no field left goes.
//! - A command on a node the remote command deleted, or whose id the remote command created again
//!   (my steps belong to the old node), goes.
//! - `AddNode` (the undo of a delete): a node whose parent the remote command deleted is attached to
//!   the nearest ancestor that survives. A remote `UpdateNode`, `Reparent` or `Reorder` of that node
//!   is written into the snapshot: the node comes back with the other actor's edit. This only happens
//!   in a replica, whose confirmed state still holds the node while my delete is pending.
//! - `Reparent` to a parent the remote command deleted goes to the nearest surviving ancestor, with no
//!   `pos`; if the node itself was deleted, the step goes. This holds for undo and redo alike.
//! - `Reparent` writes the parent and the `pos` together, so a remote `Reparent` or `Reorder` of the
//!   same node beats my `Reorder`, and a remote `Reparent` beats my `Reparent`. A remote `Reorder` writes
//!   only `pos`: my `Reparent` keeps its parent and loses its `pos`, but only when the remote `pos` was
//!   written among the siblings of the same parent (`pos` has no meaning across parents).

use crate::history::{History, Rec};
use crate::patch_ops::{drop_fields, patch_is_empty};
use meldframe_protocol::Command;
use std::collections::HashSet;

/// What the remote command removed, read from the tree before it is applied.
#[derive(Debug, Default)]
pub struct RemoteCtx {
    /// The deleted node and its descendants.
    pub removed: HashSet<String>,
    /// The parent of the deleted node: the nearest ancestor that survives.
    pub survivor: Option<String>,
    /// For a remote `Reorder`: the node and the parent it was reordered in (where its `pos` was written).
    pub reorder_space: Option<(String, Option<String>)>,
}

/// Rewrite one list of commands against the remote command `r`, which is ordered after all of them.
fn xform_list(list: &mut Vec<Command>, r: &Command, ctx: &RemoteCtx) {
    for c in std::mem::take(list) {
        match c {
            Command::UpdateNode { id, mut patch } => match r {
                Command::UpdateNode { id: rid, patch: rp } if *rid == id => {
                    drop_fields(&mut patch, rp);
                    if !patch_is_empty(&patch) {
                        list.push(Command::UpdateNode { id, patch });
                    }
                }
                Command::RemoveNode { .. } if ctx.removed.contains(&id) => {}
                Command::AddNode { node, .. } if node.id == id => {}
                _ => list.push(Command::UpdateNode { id, patch }),
            },
            Command::AddNode { mut node, index } => {
                match r {
                    Command::AddNode { node: rn, .. } if rn.id == node.id => continue,
                    // The snapshot is the node as I will add it again: it takes the remote write too.
                    Command::UpdateNode { id: rid, patch: rp } if *rid == node.id => {
                        node.apply_patch(rp)
                    }
                    Command::Reparent {
                        id: rid,
                        parent_id,
                        pos,
                    } if *rid == node.id => {
                        node.parent_id = parent_id.clone();
                        if let Some(p) = pos {
                            node.pos = p.clone();
                        }
                    }
                    Command::Reorder {
                        id: rid,
                        pos: Some(p),
                        ..
                    } if *rid == node.id => node.pos = p.clone(),
                    Command::RemoveNode { .. } => {
                        if ctx.removed.contains(&node.id) {
                            continue;
                        }
                        if node
                            .parent_id
                            .as_ref()
                            .is_some_and(|p| ctx.removed.contains(p))
                        {
                            node.parent_id = ctx.survivor.clone();
                        }
                    }
                    _ => {}
                }
                list.push(Command::AddNode { node, index });
            }
            Command::RemoveNode { id } => {
                let deleted_again =
                    matches!(r, Command::RemoveNode { .. }) && ctx.removed.contains(&id);
                let recreated = matches!(r, Command::AddNode { node, .. } if node.id == id);
                if !deleted_again && !recreated {
                    list.push(Command::RemoveNode { id });
                }
            }
            Command::Reparent { id, parent_id, pos } => match r {
                Command::Reparent { id: rid, .. } if *rid == id => {}
                Command::Reorder { id: rid, .. } if *rid == id => {
                    let same_space = ctx
                        .reorder_space
                        .as_ref()
                        .is_some_and(|(sid, space)| *sid == id && *space == parent_id);
                    let pos = if same_space { None } else { pos };
                    list.push(Command::Reparent { id, parent_id, pos });
                }
                Command::AddNode { node, .. } if node.id == id => {}
                Command::RemoveNode { .. } if ctx.removed.contains(&id) => {}
                // The parent is gone: go under its nearest surviving ancestor. Its `pos` was written
                // among other siblings, so it is not kept.
                Command::RemoveNode { .. }
                    if parent_id.as_ref().is_some_and(|p| ctx.removed.contains(p)) =>
                {
                    let parent_id = ctx.survivor.clone();
                    list.push(Command::Reparent {
                        id,
                        parent_id,
                        pos: None,
                    });
                }
                _ => list.push(Command::Reparent { id, parent_id, pos }),
            },
            Command::Reorder {
                id,
                before,
                after,
                pos,
            } => match r {
                Command::Reparent { id: rid, .. } | Command::Reorder { id: rid, .. }
                    if *rid == id => {}
                Command::AddNode { node, .. } if node.id == id => {}
                Command::RemoveNode { .. } if ctx.removed.contains(&id) => {}
                _ => list.push(Command::Reorder {
                    id,
                    before,
                    after,
                    pos,
                }),
            },
        }
    }
}

impl History {
    /// Rewrite my steps against a remote command that is ordered after them. A pending rec is skipped.
    pub fn rewrite(&mut self, cmd: &Command, ctx: &RemoteCtx) {
        let held = self.held_recs();
        let skip = |r: &&mut Rec| r.pending || held.contains(&r.id);
        for rec in self.recs_mut().filter(|r| !skip(r)) {
            xform_list(&mut rec.do_, cmd, ctx);
            xform_list(&mut rec.undo_, cmd, ctx);
        }
        self.prune();
    }
}
