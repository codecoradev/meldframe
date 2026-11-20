//! The bind picker of a field: the variables that fit one field of the selected node, found while the picker is open
//! and never otherwise. View state: nothing is written to the file and the undo history does not change.

use crate::app::App;
use crate::document::refuse;
use meldframe_app_model::{BindField, Picker};
use meldframe_app_parts::picker::picker_view;
use meldframe_app_parts::variables_panel::bindable;

/// The longest search text, in characters.
const MAX_QUERY: usize = 200;

impl App {
    /// How many times the picker was built (a test seam: it must stay as it is while no picker is open).
    #[doc(hidden)]
    pub fn picker_builds(&self) -> u64 {
        self.panel.picker_builds
    }

    pub(crate) fn open_picker(&mut self, id: String, field: BindField) {
        let Some(node) = self.nodes().iter().find(|n| n.id == id) else {
            self.state.picker = None;
            let _ = refuse(&mut self.events, "not_found");
            return;
        };
        let code = if self.state.selection != [id.clone()] {
            Some("not_selected")
        } else if !bindable(node.kind, field) {
            Some("not_bindable")
        } else {
            None
        };
        if let Some(code) = code {
            self.state.picker = None;
            let _ = refuse(&mut self.events, code);
            return;
        }
        let query = String::new();
        self.state.picker = Some(self.picker_view(id, field, query));
        self.panel.picker_builds += 1;
    }

    pub(crate) fn set_picker_query(&mut self, query: String) {
        let Some(p) = &self.state.picker else { return };
        let query: String = query.trim().chars().take(MAX_QUERY).collect();
        if p.query != query {
            let (id, field) = (p.id.clone(), p.field);
            self.state.picker = Some(self.picker_view(id, field, query));
            self.panel.picker_builds += 1;
        }
    }

    /// Bring the open picker in line with the document, the modes and the selection; close it when its node is no
    /// longer the one selected. Nothing is built while it is closed.
    pub(crate) fn refresh_picker(&mut self) {
        let Some(p) = &self.state.picker else { return };
        let still = self.state.selection == [p.id.clone()]
            && self
                .nodes()
                .iter()
                .any(|n| n.id == p.id && bindable(n.kind, p.field));
        if !still {
            self.state.picker = None;
            return;
        }
        let (id, field, query) = (p.id.clone(), p.field, p.query.clone());
        let view = self.picker_view(id, field, query);
        self.panel.picker_builds += 1;
        if self.state.picker.as_ref() != Some(&view) {
            self.state.picker = Some(view);
        }
    }

    fn picker_view(&self, id: String, field: BindField, query: String) -> Picker {
        picker_view(self.vars(), &self.choice(), self.nodes(), id, field, query)
    }
}
