//! A variant of a frame (E5.4, FR-21): a copy of the frame and its whole subtree, with new ids, placed beside the
//! original under the same parent, so two takes can be compared. [`variant_of`] only builds the commands: the
//! caller applies them as one step (the editor as one undo, an agent tool as one atomic batch), through the gate.
//!
//! The commands are `addNode`s in an order that applies on the file as it is: the root first, then each node after
//! its parent, siblings in their order. Every node keeps all its properties and its sibling position; only its id
//! and parent change, and the root also its place (to the right of its rightmost sibling, [`GAP`] apart, at the
//! original's height), its position among its siblings (last) and its name (the original's, cut to fit the name
//! limit, followed by "variant"). The commands are applied to a copy of the file first: a variant that would not
//! fit a limit (nodes, coordinates) is refused whole, so nothing is ever applied half-way.

use meldframe_check::{ApplyError, Limits};
use meldframe_protocol::{Command, Node, NodeKind};
use std::collections::{HashMap, HashSet};

/// The room between the original and its variant, in pixels.
pub const GAP: f64 = 40.0;

/// Why a variant cannot be made.
#[derive(Debug, Clone, PartialEq)]
pub enum VariantError {
    /// No node with this id.
    NotFound,
    /// The node is not a frame.
    NotAFrame,
    /// The copy would not fit the file (a limit of nodes or coordinates): nothing is built.
    Apply(ApplyError),
}

impl VariantError {
    pub fn code(&self) -> &'static str {
        match self {
            VariantError::NotFound => "E_NODE_NOT_FOUND",
            VariantError::NotAFrame => "E_NOT_A_FRAME",
            VariantError::Apply(e) => e.code(),
        }
    }
}

impl std::fmt::Display for VariantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VariantError::NotFound => write!(f, "no node with this id"),
            VariantError::NotAFrame => write!(f, "only a frame has a variant"),
            VariantError::Apply(e) => write!(f, "the variant does not fit the file: {e}"),
        }
    }
}

/// The commands of a variant and the id of its root.
#[derive(Debug, Clone)]
pub struct Variant {
    pub root: String,
    pub commands: Vec<Command>,
}

/// The variant of frame `id` in `nodes`. `fresh` gives candidate ids; one the file already has, or that this variant
/// already took, is skipped, so the caller can pass any source (a counter, keys derived from a call). The commands
/// are checked by applying them, in order, to a copy of `nodes` under `limits`.
pub fn variant_of(
    nodes: &[Node],
    id: &str,
    mut fresh: impl FnMut() -> String,
    limits: &Limits,
) -> Result<Variant, VariantError> {
    let original = nodes
        .iter()
        .find(|n| n.id == id)
        .ok_or(VariantError::NotFound)?;
    if original.kind != NodeKind::Frame {
        return Err(VariantError::NotAFrame);
    }
    let taken: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut made: HashSet<String> = HashSet::new();
    let mut next_id = || loop {
        let candidate = fresh();
        if !taken.contains(candidate.as_str()) && made.insert(candidate.clone()) {
            return candidate;
        }
    };
    let mut children: HashMap<&str, Vec<&Node>> = HashMap::new();
    for n in nodes {
        if let Some(p) = n.parent_id.as_deref() {
            children.entry(p).or_default().push(n);
        }
    }
    for list in children.values_mut() {
        list.sort_by(|a, b| a.pos.cmp(&b.pos).then_with(|| a.id.cmp(&b.id)));
    }
    let mut root = original.clone();
    let root_id = next_id();
    root.id = root_id.clone();
    // Right of every sibling (the original among them), so the copy lands on none of them.
    let right = nodes
        .iter()
        .filter(|n| n.parent_id == original.parent_id)
        .map(|n| n.x + n.w)
        .fold(original.x + original.w, f64::max);
    root.x = right + GAP;
    root.pos = String::new();
    const SUFFIX: &str = " variant";
    let keep = limits.max_name.saturating_sub(SUFFIX.chars().count());
    let base: String = original.name.chars().take(keep).collect();
    let name = if base.is_empty() {
        // No room for the suffix (or no name): the original's name, or "Variant", cut to the limit.
        let plain = if original.name.is_empty() {
            "Variant"
        } else {
            &original.name
        };
        plain.to_owned()
    } else {
        format!("{base}{SUFFIX}")
    };
    root.name = name.chars().take(limits.max_name).collect();
    let mut commands = vec![Command::AddNode {
        node: root,
        index: None,
    }];
    // Breadth first from the original: each parent is added before its children.
    let mut queue = vec![(original.id.as_str(), root_id.clone())];
    let mut at = 0;
    while at < queue.len() {
        let (old, new) = queue[at].clone();
        at += 1;
        for kid in children.get(old).map(Vec::as_slice).unwrap_or_default() {
            let mut copy = (*kid).clone();
            copy.id = next_id();
            copy.parent_id = Some(new.clone());
            queue.push((kid.id.as_str(), copy.id.clone()));
            commands.push(Command::AddNode {
                node: copy,
                index: None,
            });
        }
    }
    let mut copy = nodes.to_vec();
    for c in &commands {
        meldframe_core::apply_command(&mut copy, c, limits).map_err(VariantError::Apply)?;
    }
    Ok(Variant {
        root: root_id,
        commands,
    })
}
