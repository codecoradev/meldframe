//! The rows of the Layers panel, built from the nodes, the collapsed ids, the selection and the last actors: the tree of
//! the page, front node first, with every name made safe to show (rule U9), so a front end draws `AppState::layers` and
//! never reads a name from the file.

use meldframe_app_model::LayerRow;
use meldframe_order::sib_cmp;
use meldframe_protocol::Node;
use meldframe_text::{MAX_LABEL, label};
use std::collections::{BTreeMap, HashMap, HashSet};

/// The rows to show. A collapsed node keeps its row and hides everything below it. A node whose parent is not in the
/// file is taken for a node of the page, as the render step takes it; a node that is its own ancestor (a file that is
/// not valid) has no row, as it has no box.
pub fn layer_rows(
    nodes: &[Node],
    collapsed: &[String],
    selection: &[String],
    last_actor: &BTreeMap<String, String>,
) -> Vec<LayerRow> {
    let ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut kids: HashMap<Option<&str>, Vec<&Node>> = HashMap::new();
    for n in nodes {
        let parent = n.parent_id.as_deref().filter(|p| ids.contains(p));
        kids.entry(parent).or_default().push(n);
    }
    for list in kids.values_mut() {
        list.sort_by(|a, b| sib_cmp(a, b));
    }
    let hidden: HashSet<&str> = collapsed.iter().map(String::as_str).collect();
    let picked: HashSet<&str> = selection.iter().map(String::as_str).collect();
    let mut rows = Vec::new();
    // The last sibling is the front one and comes first, so each level is pushed in order and popped from the end.
    let mut stack: Vec<(&Node, u32)> = kids
        .get(&None)
        .map(|roots| roots.iter().map(|n| (*n, 0)).collect())
        .unwrap_or_default();
    while let Some((n, depth)) = stack.pop() {
        let below = kids.get(&Some(n.id.as_str()));
        let folded = hidden.contains(n.id.as_str());
        rows.push(LayerRow {
            id: n.id.clone(),
            name: label(&n.name, MAX_LABEL),
            kind: n.kind,
            depth,
            has_children: below.is_some_and(|k| !k.is_empty()),
            collapsed: folded,
            selected: picked.contains(n.id.as_str()),
            actor: last_actor.get(&n.id).cloned(),
        });
        if let Some(below) = below.filter(|_| !folded) {
            stack.extend(below.iter().map(|c| (*c, depth + 1)));
        }
    }
    rows
}
