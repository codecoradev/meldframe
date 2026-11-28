//! What the view of the Variables panel was built from, to tell whether it must be built again, and the one node the
//! selection names. A dispatch that changes none of these (a move, a resize, a timer) leaves the view as it is: the
//! comparison reads the file but allocates only when something differs.

use super::variables_panel::bound_fields;
use meldframe_app_model::{Panel, Panels};
use meldframe_protocol::{Node, NodeKind, Variables};
use std::collections::BTreeMap;

/// The node a selection of exactly one id names.
pub fn selected_node<'a>(selection: &[String], nodes: &'a [Node]) -> Option<&'a Node> {
    match selection {
        [id] => nodes.iter().find(|n| n.id == *id),
        _ => None,
    }
}

/// What the view was built from.
#[derive(Debug, PartialEq)]
pub struct PanelKey {
    query: String,
    modes: BTreeMap<String, String>,
    selection: Vec<String>,
    kind: Option<NodeKind>,
    vars: Variables,
    /// Every bound field of every node: `(node, field, variable)`; the use counts and the buttons read them.
    bound: Vec<(String, &'static str, String)>,
}

impl PanelKey {
    pub fn new(
        query: &str,
        modes: &BTreeMap<String, String>,
        selection: &[String],
        vars: &Variables,
        nodes: &[Node],
    ) -> Self {
        PanelKey {
            query: query.to_string(),
            modes: modes.clone(),
            selection: selection.to_vec(),
            kind: selected_node(selection, nodes).map(|n| n.kind),
            vars: vars.clone(),
            bound: bound_fields(nodes),
        }
    }

    /// Whether the view built from this key is still the one to show.
    pub fn is_current(
        &self,
        query: &str,
        modes: &BTreeMap<String, String>,
        selection: &[String],
        vars: &Variables,
        nodes: &[Node],
    ) -> bool {
        self.query == query
            && self.modes == *modes
            && self.selection == selection
            && self.kind == selected_node(selection, nodes).map(|n| n.kind)
            && self.vars == *vars
            && self.bound == bound_fields(nodes)
    }
}

/// Whether a panel is open.
pub fn is_open(panels: &Panels, panel: Panel) -> bool {
    match panel {
        Panel::Layers => panels.layers,
        Panel::Properties => panels.properties,
        Panel::Variables => panels.variables,
    }
}

/// Open or close a panel.
pub fn set_open(panels: &mut Panels, panel: Panel, open: bool) {
    match panel {
        Panel::Layers => panels.layers = open,
        Panel::Properties => panels.properties = open,
        Panel::Variables => panels.variables = open,
    }
}

/// Flip `id` in the list of collapsed nodes; true when it is collapsed now.
pub fn toggle_listed(list: &mut Vec<String>, id: &str) -> bool {
    match list.iter().position(|c| c == id) {
        Some(at) => {
            list.remove(at);
            false
        }
        None => {
            list.push(id.to_string());
            true
        }
    }
}
