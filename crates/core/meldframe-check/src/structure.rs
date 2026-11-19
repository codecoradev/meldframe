//! Checks of a command against the structure of the file: ids, parents, cycles, depth and sibling anchors.

use crate::validate::{check_node, check_patch, check_pos};
use crate::{ApplyError, Limits};
use meldframe_protocol::{Command, Node, NodeKind, Variables};
use meldframe_variables::{
    VarError, check_node_bindings, check_patch_bindings, check_variable_command,
    is_variable_command,
};
use std::collections::HashSet;

/// Index of the node with this id.
pub fn position(nodes: &[Node], id: &str) -> Option<usize> {
    nodes.iter().position(|n| n.id == id)
}

/// Depth of a node (a root is 0). Guarded against cycles in corrupt data.
fn depth_of(nodes: &[Node], mut idx: usize) -> usize {
    let mut d = 0;
    while let Some(pid) = nodes[idx].parent_id.as_deref() {
        match position(nodes, pid) {
            Some(p) => idx = p,
            None => break,
        }
        d += 1;
        if d > nodes.len() {
            break;
        }
    }
    d
}

/// Height of a subtree (a leaf is 0).
fn subtree_height(nodes: &[Node], root_id: &str) -> usize {
    let mut level: HashSet<&str> = HashSet::from([root_id]);
    let mut h = 0;
    loop {
        let next: HashSet<&str> = nodes
            .iter()
            .filter(|n| n.parent_id.as_deref().is_some_and(|p| level.contains(p)))
            .map(|n| n.id.as_str())
            .collect();
        if next.is_empty() || h > nodes.len() {
            return h;
        }
        h += 1;
        level = next;
    }
}

/// The parent must exist, be a frame and leave room for `extra_depth` more levels below it.
fn check_parent(
    nodes: &[Node],
    parent: &str,
    l: &Limits,
    extra_depth: usize,
) -> Result<usize, ApplyError> {
    let pi =
        position(nodes, parent).ok_or_else(|| ApplyError::ParentNotFound(parent.to_string()))?;
    if nodes[pi].kind != NodeKind::Frame {
        return Err(ApplyError::ParentNotFrame(parent.to_string()));
    }
    if depth_of(nodes, pi) + 1 + extra_depth > l.max_depth {
        return Err(ApplyError::TooDeep(l.max_depth));
    }
    Ok(pi)
}

/// Whether `ancestor` is `start` or one of its ancestors.
fn is_ancestor_or_self(nodes: &[Node], ancestor: &str, start: usize) -> bool {
    let mut cur = start;
    for _ in 0..=nodes.len() {
        if nodes[cur].id == ancestor {
            return true;
        }
        match nodes[cur]
            .parent_id
            .as_deref()
            .and_then(|p| position(nodes, p))
        {
            Some(i) => cur = i,
            None => return false,
        }
    }
    false
}

fn check_reorder(
    nodes: &[Node],
    i: usize,
    (before, after, pos): (&Option<String>, &Option<String>, &Option<String>),
    l: &Limits,
) -> Result<(), ApplyError> {
    let id = &nodes[i].id;
    let given = before.is_some() as u8 + after.is_some() as u8 + pos.is_some() as u8;
    if given > 1 {
        return Err(ApplyError::BadReorder(
            "at most one of before, after and pos".into(),
        ));
    }
    if let Some(p) = pos {
        check_pos(p, l)?;
    }
    for anchor in [before, after].into_iter().flatten() {
        let ai = position(nodes, anchor)
            .ok_or_else(|| ApplyError::BadReorder(format!("anchor not found: {anchor}")))?;
        if anchor == id || nodes[ai].parent_id != nodes[i].parent_id {
            return Err(ApplyError::BadReorder(format!(
                "anchor is not a sibling: {anchor}"
            )));
        }
    }
    Ok(())
}

/// Validate a command against `nodes` alone, for a caller that holds no variables: a binding names a variable
/// that does not exist, and a variable command is refused as `NoVariables`. See [`check_command_with`].
pub fn check_command(nodes: &[Node], cmd: &Command, l: &Limits) -> Result<(), ApplyError> {
    if is_variable_command(cmd) {
        return Err(ApplyError::Variable(VarError::NoVariables));
    }
    check_command_with(nodes, &Variables::default(), cmd, l)
}

/// Validate a command against `nodes` and `vars` without changing them.
pub fn check_command_with(
    nodes: &[Node],
    vars: &Variables,
    cmd: &Command,
    l: &Limits,
) -> Result<(), ApplyError> {
    match cmd {
        Command::UpdateNode { id, patch } => {
            let i = position(nodes, id).ok_or_else(|| ApplyError::NotFound(id.clone()))?;
            if matches!(patch.image, Some(Some(_))) && nodes[i].kind == NodeKind::Text {
                return Err(ApplyError::ImageOnText);
            }
            if matches!(patch.gradient, Some(Some(_))) && nodes[i].kind == NodeKind::Text {
                return Err(ApplyError::GradientOnText);
            }
            check_patch(patch, l)?;
            check_patch_bindings(vars, patch).map_err(ApplyError::Variable)
        }
        Command::AddNode { node, .. } => {
            check_node(node, l)?;
            check_node_bindings(vars, node).map_err(ApplyError::Variable)?;
            if position(nodes, &node.id).is_some() {
                return Err(ApplyError::DuplicateId(node.id.clone()));
            }
            if nodes.len() + 1 > l.max_nodes {
                return Err(ApplyError::TooManyNodes(l.max_nodes));
            }
            if let Some(p) = node.parent_id.as_deref() {
                check_parent(nodes, p, l, 0)?;
            }
            Ok(())
        }
        Command::RemoveNode { id } => {
            position(nodes, id).ok_or_else(|| ApplyError::NotFound(id.clone()))?;
            Ok(())
        }
        Command::Reorder {
            id,
            before,
            after,
            pos,
        } => {
            let i = position(nodes, id).ok_or_else(|| ApplyError::NotFound(id.clone()))?;
            check_reorder(nodes, i, (before, after, pos), l)
        }
        Command::Reparent { id, parent_id, pos } => {
            position(nodes, id).ok_or_else(|| ApplyError::NotFound(id.clone()))?;
            if let Some(p) = pos {
                check_pos(p, l)?;
            }
            if let Some(p) = parent_id.as_deref() {
                if p == id {
                    return Err(ApplyError::Cycle);
                }
                let pi = check_parent(nodes, p, l, subtree_height(nodes, id))?;
                // The new parent must not be a descendant of the node being moved.
                if is_ancestor_or_self(nodes, id, pi) {
                    return Err(ApplyError::Cycle);
                }
            }
            Ok(())
        }
        _ => {
            check_variable_command(nodes, vars, cmd, &l.var_limits()).map_err(ApplyError::Variable)
        }
    }
}
