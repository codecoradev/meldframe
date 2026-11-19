//! The check of a whole stored file, run when a file is loaded. `check_command` guards each change; this
//! guards data that an older build, a bug or a hand edit may have left in the store. It never repairs:
//! the first problem found is returned and the caller decides (the server refuses to open the file and
//! keeps the bytes). It is meant for a snapshot that the old-file migration has already run on: sibling
//! `pos` must be present and unique here, which a live tree after concurrent edits does not promise.

use crate::validate::check_node;
use crate::{ApplyError, Limits};
use meldframe_protocol::{Node, NodeKind, Variables};
use meldframe_variables::{check_file_bindings, check_variables};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Why a stored file was refused at load.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadError {
    /// One node, or the file as a whole (`node` is `None`), breaks a rule a command would also enforce.
    /// The code is the code of `error`.
    Node {
        node: Option<String>,
        error: ApplyError,
    },
    /// A node has no `pos`. The loader migrates an old file first, so this means a caller skipped that.
    MissingPos(String),
    /// Two siblings (children of `parent`, or two roots) have the same `pos`: their order is undefined.
    DuplicatePos { parent: Option<String>, pos: String },
}

impl LoadError {
    /// The stable machine code (`docs/error-codes.md`). A wrapped `ApplyError` keeps its own code.
    pub fn code(&self) -> &'static str {
        match self {
            LoadError::Node { error, .. } => error.code(),
            LoadError::MissingPos(_) => "E_MISSING_POS",
            LoadError::DuplicatePos { .. } => "E_DUPLICATE_POS",
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Node {
                node: Some(id),
                error,
            } => write!(f, "node {id}: {error}"),
            LoadError::Node { node: None, error } => write!(f, "file: {error}"),
            LoadError::MissingPos(id) => write!(f, "node {id} has no pos"),
            LoadError::DuplicatePos { parent, pos } => write!(
                f,
                "siblings under {} share the pos {pos:?}",
                parent.as_deref().unwrap_or("the root")
            ),
        }
    }
}

impl std::error::Error for LoadError {}

fn at(id: &str, error: ApplyError) -> LoadError {
    LoadError::Node {
        node: Some(id.to_string()),
        error,
    }
}

/// Check a whole file: the node count, every node with `check_node`, unique ids, parents that exist and
/// are frames (so a text node has no children), no cycle (a node that is its own parent included), the
/// depth, and a valid, unique `pos` among siblings. Nothing is changed.
pub fn check_file(nodes: &[Node], l: &Limits) -> Result<(), LoadError> {
    check_file_with(nodes, &Variables::default(), l)
}

/// Like [`check_file`] for a file with variables: the variables are checked first, then the nodes, then every
/// binding of a node must name a variable of the right type. A problem in a variable is a
/// [`LoadError::Node`] with the variable's id (`None` when it is about the whole set).
pub fn check_file_with(nodes: &[Node], vars: &Variables, l: &Limits) -> Result<(), LoadError> {
    let var = |(id, e): (String, _)| LoadError::Node {
        node: Some(id).filter(|id| !id.is_empty()),
        error: ApplyError::Variable(e),
    };
    check_variables(vars, &l.var_limits()).map_err(var)?;
    check_file_nodes(nodes, l)?;
    check_file_bindings(nodes, vars).map_err(var)
}

fn check_file_nodes(nodes: &[Node], l: &Limits) -> Result<(), LoadError> {
    if nodes.len() > l.max_nodes {
        return Err(LoadError::Node {
            node: None,
            error: ApplyError::TooManyNodes(l.max_nodes),
        });
    }
    let mut index: HashMap<&str, usize> = HashMap::with_capacity(nodes.len());
    for (i, n) in nodes.iter().enumerate() {
        check_node(n, l).map_err(|e| at(&n.id, e))?;
        if index.insert(n.id.as_str(), i).is_some() {
            return Err(at(&n.id, ApplyError::DuplicateId(n.id.clone())));
        }
    }
    for n in nodes {
        if let Some(p) = n.parent_id.as_deref() {
            let pi = *index
                .get(p)
                .ok_or_else(|| at(&n.id, ApplyError::ParentNotFound(p.to_string())))?;
            if nodes[pi].kind != NodeKind::Frame {
                return Err(at(&n.id, ApplyError::ParentNotFrame(p.to_string())));
            }
        }
    }
    check_depths(nodes, &index, l)?;
    check_positions(nodes)
}

/// Walk up from every node. A walk that meets a node already on its own path is a cycle; one that meets a
/// node with a known depth stops there.
fn check_depths(nodes: &[Node], index: &HashMap<&str, usize>, l: &Limits) -> Result<(), LoadError> {
    let mut depth: Vec<Option<usize>> = vec![None; nodes.len()];
    for start in 0..nodes.len() {
        let mut path: Vec<usize> = Vec::new();
        let mut on_path: HashSet<usize> = HashSet::new();
        let mut cur = start;
        let base = loop {
            if let Some(d) = depth[cur] {
                break d + 1;
            }
            if !on_path.insert(cur) {
                return Err(at(&nodes[cur].id, ApplyError::Cycle));
            }
            path.push(cur);
            match nodes[cur].parent_id.as_deref() {
                Some(p) => cur = index[p],
                None => break 0,
            }
        };
        for (k, &i) in path.iter().rev().enumerate() {
            let d = base + k;
            if d > l.max_depth {
                return Err(at(&nodes[i].id, ApplyError::TooDeep(l.max_depth)));
            }
            depth[i] = Some(d);
        }
    }
    Ok(())
}

fn check_positions(nodes: &[Node]) -> Result<(), LoadError> {
    let mut seen: HashSet<(Option<&str>, &str)> = HashSet::with_capacity(nodes.len());
    for n in nodes {
        if n.pos.is_empty() {
            return Err(LoadError::MissingPos(n.id.clone()));
        }
        if !seen.insert((n.parent_id.as_deref(), n.pos.as_str())) {
            return Err(LoadError::DuplicatePos {
                parent: n.parent_id.clone(),
                pos: n.pos.clone(),
            });
        }
    }
    Ok(())
}
