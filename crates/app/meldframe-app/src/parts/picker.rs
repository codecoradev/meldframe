//! The rows of the bind picker of a field: the variables that fit one field of a node, with the search applied.

use super::variables_panel::{bindable, field_name, value_text};
use meldframe_app_model::{BindField, Picker, PickerRow};
use meldframe_protocol::{Node, VarType, Variables};
use meldframe_text::{MAX_LABEL, label};
use meldframe_variables::{ModeChoice, Resolver, node_bindings};

/// The most rows the picker lists; `Picker::total` says how many variables fit the field.
const MAX_ROWS: usize = 50;

/// The type of variable a field takes: colours for a fill, numbers for a radius or an opacity.
fn type_of(field: BindField) -> VarType {
    match field {
        BindField::Fill => VarType::Color,
        BindField::Radius | BindField::Opacity => VarType::Number,
    }
}

/// The picker as it is built from the file, the modes on view and the search text.
pub fn picker_view(
    vars: &Variables,
    choice: &ModeChoice,
    nodes: &[Node],
    id: String,
    field: BindField,
    query: String,
) -> Picker {
    let resolver = Resolver::new(vars, choice);
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let bound = nodes
        .iter()
        .find(|n| n.id == id)
        .map(|n| {
            node_bindings(n)
                .into_iter()
                .find(|b| b.field == field_name(field))
                .map(|b| b.var.to_string())
        })
        .unwrap_or_default();
    let (mut total, mut rows) = (0, Vec::new());
    for v in vars.variables.iter().filter(|v| v.kind == type_of(field)) {
        total += 1;
        let collection = vars
            .collections
            .iter()
            .find(|c| c.id == v.collection)
            .map_or_else(String::new, |c| c.name.clone());
        let resolved = resolver
            .value(&v.id)
            .map(|x| value_text(&x))
            .unwrap_or_default();
        if !words.is_empty() {
            let hay = format!("{} {collection} {resolved}", v.name).to_lowercase();
            if !words.iter().all(|w| hay.contains(w)) {
                continue;
            }
        }
        if rows.len() < MAX_ROWS {
            rows.push(PickerRow {
                id: v.id.clone(),
                name: label(&v.name, MAX_LABEL),
                collection: label(&collection, MAX_LABEL),
                kind: v.kind,
                resolved,
                bound: bound.as_deref() == Some(v.id.as_str()),
            });
        }
    }
    Picker {
        id,
        field,
        kind: type_of(field),
        query,
        total,
        rows,
    }
}

/// The code a request to open the picker on a field of node `id` is refused with, when it is: no such node, the node is
/// not the one selected, or the field cannot be bound on this kind of node.
pub fn open_refusal(
    nodes: &[Node],
    selection: &[String],
    id: &str,
    field: BindField,
) -> Option<&'static str> {
    let node = nodes.iter().find(|n| n.id == id);
    match node {
        None => Some("not_found"),
        Some(_) if !matches!(selection, [only] if only == id) => Some("not_selected"),
        Some(n) if !bindable(n.kind, field) => Some("not_bindable"),
        Some(_) => None,
    }
}

/// Whether the picker open on a field of node `id` still has what it needs: the node is the one selected, and the
/// field can be bound on it.
pub fn is_open_valid(nodes: &[Node], selection: &[String], id: &str, field: BindField) -> bool {
    matches!(selection, [only] if only == id)
        && nodes.iter().any(|n| n.id == id && bindable(n.kind, field))
}
