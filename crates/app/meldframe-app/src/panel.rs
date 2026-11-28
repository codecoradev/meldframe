//! The Variables panel (E3.4): the rows it lists, its search, and the actions that edit a value or bind a field, where
//! every rule is here; a front end draws `AppState::variables` and sends the actions, and the checks of a value, an
//! alias or a binding are the ones of `meldframe-variables`, run when the command is applied.

use crate::{app::App, document::refuse, event::Event, view_cache::PanelCache};
use meldframe_app_model::{BindField, VariableValue, VariablesPanel};
use meldframe_app_parts::binding::{bind_patch, variable_bind};
use meldframe_app_parts::panel::{PanelKey, selected_node};
use meldframe_app_parts::variables_panel::variables_view;
use meldframe_protocol::Command;
use meldframe_sync::SubmitError;

/// The longest search text, in characters.
const MAX_QUERY: usize = 200;

impl PanelCache {
    fn is_current(&self, a: &App) -> bool {
        self.key.as_ref().is_some_and(|k| {
            k.is_current(
                &a.state.variables.query,
                &a.state.modes,
                &a.state.selection,
                a.vars(),
                a.nodes(),
            )
        })
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
            selected_node(&self.state.selection, self.nodes()),
            &query,
        );
        self.panel.builds += 1;
        self.panel.key = Some(PanelKey::new(
            &query,
            &self.state.modes,
            &self.state.selection,
            self.vars(),
            self.nodes(),
        ));
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
        let new = match variable_bind(var.kind, value) {
            Ok(new) => new,
            Err(code) => return refuse(&mut self.events, code),
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

    /// The text of a value field: a literal as `set_variable_value` reads it, and nothing for empty text.
    pub(crate) fn set_variable_value_text(
        &mut self,
        variable: String,
        mode: String,
        text: String,
    ) -> Result<(), SubmitError> {
        if text.trim().is_empty() {
            return Ok(());
        }
        self.set_variable_value(variable, mode, VariableValue::Literal { text })
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
        let patch = match bind_patch(node, field, variable, self.vars(), &self.choice()) {
            Ok(Some(patch)) => patch,
            // Already so: bind the same variable again, or unbind a field that is a literal.
            Ok(None) => return Ok(()),
            Err(code) => return refuse(&mut self.events, code),
        };
        self.replica.apply(&Command::UpdateNode {
            id: id.to_string(),
            patch,
        })
    }
}
