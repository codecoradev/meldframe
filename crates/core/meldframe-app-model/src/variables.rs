//! The Variables panel as plain data (E3.4): the rows a front end draws and the inputs it sends. The rules that
//! fill and act on them are in `meldframe-app`.

use meldframe_protocol::{Mode, VarType};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

impl VariablesPanel {
    /// The panel with nothing listed, for this search text.
    pub fn default_for(query: &str) -> Self {
        VariablesPanel {
            query: query.to_string(),
            ..Default::default()
        }
    }
}

/// The Variables panel: the search, the counts and the collections with the variables that match. Empty while the
/// panel is closed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariablesPanel {
    /// The search text, trimmed. Every word must occur in a variable's name, its collection's name, or one of its
    /// values (a literal as written, an alias as the name it points at), ignoring case.
    pub query: String,
    /// Variables in the file, and how many of them match the search.
    pub total: usize,
    pub matched: usize,
    /// The collections that hold a match (all of them while the search is empty). At most 200 variables are
    /// listed in all, so a collection can be listed with no rows when its matches (`CollectionRow::matched`) all fall
    /// beyond the cap: the search has to be narrower.
    pub collections: Vec<CollectionRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CollectionRow {
    pub id: String,
    pub name: String,
    pub modes: Vec<Mode>,
    /// The id of the mode the canvas shows for this collection.
    pub active: String,
    /// How many variables of this collection match the search. The rows listed may be fewer (the cap of the panel).
    pub matched: usize,
    pub variables: Vec<VariableRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariableRow {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub kind: VarType,
    /// One entry for each mode of the collection, in its order.
    pub values: Vec<ModeValue>,
    /// How many node fields and aliases use this variable.
    pub used_by: usize,
    /// The fields of the one selected node that this variable fits, and whether each is bound to it. Empty for no
    /// selection or several.
    pub binds: Vec<BindOption>,
}

/// The value of a variable in one mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModeValue {
    pub mode: String,
    /// A literal as stored, or the name of the variable an alias points at.
    pub text: String,
    /// The id of the variable an alias points at; `None` for a literal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub alias: Option<String>,
    /// What the value comes to in this mode: the literal, or what the alias chain resolves to (empty when it does not).
    pub resolved: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BindOption {
    pub field: BindField,
    pub bound: bool,
}

/// A node field that can hold a binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum BindField {
    Fill,
    Radius,
    Opacity,
}

/// What a person sets a variable's value to in one mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum VariableValue {
    /// A colour as `#hex` or a number, as typed.
    Literal { text: String },
    /// An alias to the variable with this id.
    Alias { variable: String },
}
