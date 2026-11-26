//! The Layers panel as plain data: the rows a front end draws, with the name already made safe to show (rule U9).

use meldframe_protocol::NodeKind;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One row of the Layers panel: a node of the page, front node first, children under their parent. Every text in it is
/// safe to draw as it is. The node's own name is in the file (`App::nodes`), as written: it is never drawn from
/// there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayerRow {
    /// The id of the node: what every action takes. It is never cleaned.
    pub id: String,
    /// The node's name made safe to show (`meldframe_text::label`, display profile): no hidden or control character, a
    /// joiner kept between emoji, white space folded, cut at 256 characters. Empty when nothing is left of it: the
    /// front end words that case as it does for a node with no name.
    pub name: String,
    pub kind: NodeKind,
    /// How deep the row sits (0 for a node on the page).
    pub depth: u32,
    pub has_children: bool,
    /// The children are hidden (`Panels::collapsed`).
    pub collapsed: bool,
    pub selected: bool,
    /// The actor of the last change to the node as the server stamped it, made safe as a lane key is
    /// (`AppState::last_actor`); none for a node nobody changed since the page loaded.
    pub actor: Option<String>,
}
