//! Validation and application of a [`Command`] to a file's nodes. One implementation serves the server
//! (native) and the client (WASM), so the same ops in the same order give the same file everywhere.
//!
//! Rule: validate first, change after. A command that fails validation leaves the file untouched.

mod order;

use meldframe_check::{ApplyError, Limits, check_command, position};
use meldframe_protocol::{Command, Node};
use order::{Want, resolve_pos};
use std::collections::HashSet;

pub use order::{normalize_positions, rebalance_commands};

/// The final `pos` of the node a command touches (`None` = unchanged), worked out against the current
/// nodes. Fails if the command is refused.
pub fn plan_command(
    nodes: &[Node],
    cmd: &Command,
    l: &Limits,
) -> Result<Option<String>, ApplyError> {
    check_command(nodes, cmd, l)?;
    Ok(match cmd {
        Command::AddNode { node, index } => {
            // An explicit pos wins over the deprecated index.
            let want = if !node.pos.is_empty() {
                Want::Pos(&node.pos)
            } else if let Some(i) = index {
                Want::Index(*i)
            } else {
                Want::End
            };
            Some(resolve_pos(
                nodes,
                node.parent_id.as_deref(),
                None,
                want,
                l,
            )?)
        }
        Command::Reparent { id, parent_id, pos } => {
            let cur = &nodes[position(nodes, id).expect("checked")];
            if cur.parent_id == *parent_id && pos.is_none() {
                None
            } else {
                let want = pos.as_deref().map_or(Want::End, Want::Pos);
                Some(resolve_pos(nodes, parent_id.as_deref(), Some(id), want, l)?)
            }
        }
        Command::Reorder {
            id,
            before,
            after,
            pos,
        } => {
            let cur = &nodes[position(nodes, id).expect("checked")];
            let want = match (before, after, pos) {
                (Some(b), _, _) => Want::Before(b),
                (_, Some(a), _) => Want::After(a),
                (_, _, Some(p)) => Want::Pos(p),
                _ => Want::End,
            };
            let resolved = resolve_pos(nodes, cur.parent_id.as_deref(), Some(id), want, l)?;
            if resolved == cur.pos {
                None
            } else {
                Some(resolved)
            }
        }
        Command::UpdateNode { .. } | Command::RemoveNode { .. } => None,
    })
}

/// Validate, then apply. If validation fails, `nodes` is unchanged.
pub fn apply_command(nodes: &mut Vec<Node>, cmd: &Command, l: &Limits) -> Result<(), ApplyError> {
    let pos = plan_command(nodes, cmd, l)?;
    apply_planned(nodes, cmd, pos);
    Ok(())
}

/// Apply a command that `plan_command` already validated and planned. History uses it to compute the
/// inverse from the state before applying.
pub fn apply_planned(nodes: &mut Vec<Node>, cmd: &Command, pos: Option<String>) {
    match cmd {
        Command::UpdateNode { id, patch } => {
            let i = position(nodes, id).expect("checked");
            nodes[i].apply_patch(patch);
        }
        Command::AddNode { node, .. } => {
            // List position carries no meaning: always append. Sibling order is `(pos, id)`.
            let mut n = node.clone();
            n.pos = pos.expect("an added node always gets a pos");
            nodes.push(n);
        }
        Command::Reparent { id, parent_id, .. } => {
            let i = position(nodes, id).expect("checked");
            nodes[i].parent_id = parent_id.clone();
            if let Some(p) = pos {
                nodes[i].pos = p;
            }
        }
        Command::Reorder { id, .. } => {
            let i = position(nodes, id).expect("checked");
            if let Some(p) = pos {
                nodes[i].pos = p;
            }
        }
        Command::RemoveNode { id } => {
            let mut doomed: HashSet<String> = HashSet::from([id.clone()]);
            let mut grew = true;
            while grew {
                grew = false;
                for n in nodes.iter() {
                    let child = n.parent_id.as_deref().is_some_and(|p| doomed.contains(p));
                    if child && doomed.insert(n.id.clone()) {
                        grew = true;
                    }
                }
            }
            nodes.retain(|n| !doomed.contains(&n.id));
        }
    }
}
