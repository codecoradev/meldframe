//! The Variables panel (E3.4): the rows it lists, its search, and the actions that edit a value or bind a field.
//! Every rule is here; a front end draws `AppState::variables` and sends the actions. The checks of a value, an
//! alias or a binding are the ones of `meldframe-variables`, run when the command is applied.

use crate::app::App;
use crate::document::refuse;
use crate::event::Event;
use meldframe_app_model::{BindField, VariableValue, VariablesPanel};
use meldframe_app_parts::variables_panel::{bindable, bound_fields, field_name, variables_view};
use meldframe_protocol::{Bind, Command, Node, NodeKind, NodePatch, Value, VarType, Variables};
use meldframe_render::Fields;
use meldframe_sync::SubmitError;
use meldframe_variables::node_bindings;
use std::collections::BTreeMap;

/// The longest search text, in characters.
const MAX_QUERY: usize = 200;

/// What the view was built from, and how often it was built. A dispatch that changes none of these (a move, a
/// resize, a timer) leaves the view as it is: the check reads the file but allocates only when something differs.
#[derive(Debug, Default)]
pub(crate) struct PanelCache {
    key: Option<Key>,
    builds: u64,
}

#[derive(Debug, PartialEq)]
struct Key {
    query: String,
    modes: BTreeMap<String, String>,
    selection: Vec<String>,
    kind: Option<NodeKind>,
    vars: Variables,
    /// Every bound field of every node: `(node, field, variable)`; the use counts and the buttons read them.
    bound: Vec<(String, &'static str, String)>,
}

impl PanelCache {
    fn is_current(&self, a: &App) -> bool {
        let Some(k) = &self.key else { return false };
        k.query == a.state.variables.query
            && k.modes == a.state.modes
            && k.selection == a.state.selection
            && k.kind == selected(a).map(|n| n.kind)
            && k.vars == *a.vars()
            && k.bound == bound_fields(a.nodes())
    }
}

fn selected(a: &App) -> Option<&Node> {
    match a.state.selection.as_slice() {
        [id] => a.nodes().iter().find(|n| n.id == *id),
        _ => None,
    }
}

impl App {
    /// How many times the view of the panel was built (a test seam: it must not grow on a dispatch that changes
    /// nothing the panel reads).
    #[doc(hidden)]
    pub fn variables_view_builds(&self) -> u64 {
        self.panel.builds
    }

    /// Search the panel. View state: no op, no undo step. The text is trimmed and cut at 200 characters.
    pub(crate) fn set_variables_query(&mut self, query: String) {
        let query: String = query.trim().chars().take(MAX_QUERY).collect();
        if self.state.variables.query != query {
            self.events.push(Event::VariablesQueryChanged {
                query: query.clone(),
            });
            self.state.variables.query = query;
        }
    }

    /// Bring `AppState::variables` in line with the document, the modes, the selection and the search. Nothing is
    /// listed while the panel is closed, and nothing is built again while what the view reads is as it was.
    pub(crate) fn refresh_variables(&mut self) {
        let query = self.state.variables.query.clone();
        if !self.state.panels.variables {
            self.panel.key = None;
            if self.state.variables != VariablesPanel::default_for(&query) {
                self.state.variables = VariablesPanel::default_for(&query);
            }
            return;
        }
        if self.panel.is_current(self) {
            return;
        }
        let view = variables_view(
            self.vars(),
            self.nodes(),
            &self.choice(),
            selected(self),
            &query,
        );
        self.panel.builds += 1;
        self.panel.key = Some(Key {
            query,
            modes: self.state.modes.clone(),
            selection: self.state.selection.clone(),
            kind: selected(self).map(|n| n.kind),
            vars: self.vars().clone(),
            bound: bound_fields(self.nodes()),
        });
        self.state.variables = view;
    }

    /// Set the value of a variable in one mode.
    pub(crate) fn set_variable_value(
        &mut self,
        variable: String,
        mode: String,
        value: VariableValue,
    ) -> Result<(), SubmitError> {
        let Some(var) = self.vars().variable(&variable) else {
            return refuse(&mut self.events, "E_VARIABLE_NOT_FOUND");
        };
        let new: Bind<Value> = match value {
            VariableValue::Alias { variable: to } => Bind::var(to),
            VariableValue::Literal { text } => match var.kind {
                VarType::Color => Bind::Lit(Value::Color(text.trim().to_string())),
                VarType::Number => match text.trim().parse::<f64>() {
                    Ok(n) => Bind::Lit(Value::Number(n)),
                    Err(_) => return refuse(&mut self.events, "invalid_value"),
                },
            },
        };
        if var.values.get(&mode) == Some(&new) {
            return Ok(());
        }
        self.replica.apply(&Command::SetVariableValue {
            id: variable,
            mode,
            value: new,
        })
    }

    /// Bind a field of a node to a variable, or let it go with the value that is painted.
    pub(crate) fn bind_property(
        &mut self,
        id: &str,
        field: BindField,
        variable: Option<String>,
    ) -> Result<(), SubmitError> {
        let Some(node) = self.nodes().iter().find(|n| n.id == id) else {
            return refuse(&mut self.events, "not_found");
        };
        if !bindable(node.kind, field) {
            return refuse(&mut self.events, "not_bindable");
        }
        let bound = node_bindings(node)
            .into_iter()
            .find(|b| b.field == field_name(field))
            .map(|b| b.var.to_string());
        if bound == variable {
            // Already so: bind the same variable again, or unbind a field that is a literal.
            return Ok(());
        }
        let mut patch = NodePatch::default();
        match variable {
            Some(v) => match field {
                BindField::Fill => patch.fill = Some(Bind::var(v)),
                BindField::Radius => patch.radius = Some(Bind::var(v)),
                BindField::Opacity => patch.opacity = Some(Bind::var(v)),
            },
            None => {
                let choice = self.choice();
                let f = Fields::new(self.vars(), &choice);
                match field {
                    BindField::Fill => patch.fill = Some(Bind::Lit(f.fill_text(&node.fill))),
                    BindField::Radius => patch.radius = Some(Bind::Lit(f.radius(&node.radius))),
                    BindField::Opacity => patch.opacity = Some(Bind::Lit(f.opacity(&node.opacity))),
                }
            }
        }
        self.replica.apply(&Command::UpdateNode {
            id: id.to_string(),
            patch,
        })
    }
}
