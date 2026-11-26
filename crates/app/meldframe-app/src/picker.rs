//! The bind picker of a field: the variables that fit one field of the selected node, found while the picker is open
//! and never otherwise. View state: nothing is written to the file and the undo history does not change.

use crate::app::App;
use crate::document::refuse;
use meldframe_app_model::{BindField, Event, Picker};
use meldframe_app_parts::picker::{is_open_valid, open_refusal, picker_view};

/// The longest search text, in characters.
const MAX_QUERY: usize = 200;

impl App {
    /// How many times the picker was built (a test seam: it must stay as it is while no picker is open).
    #[doc(hidden)]
    pub fn picker_builds(&self) -> u64 {
        self.panel.picker_builds
    }

    pub(crate) fn open_picker(&mut self, id: String, field: BindField) {
        if let Some(code) = open_refusal(self.nodes(), &self.state.selection, &id, field) {
            self.set_picker(None);
            let _ = refuse(&mut self.events, code);
            return;
        }
        let query = String::new();
        let view = self.picker_view(id, field, query);
        self.panel.picker_builds += 1;
        self.set_picker(Some(view));
    }

    pub(crate) fn set_picker_query(&mut self, query: String) {
        let Some(p) = &self.state.picker else { return };
        let query: String = query.trim().chars().take(MAX_QUERY).collect();
        if p.query != query {
            let (id, field) = (p.id.clone(), p.field);
            let view = self.picker_view(id, field, query);
            self.panel.picker_builds += 1;
            self.set_picker(Some(view));
        }
    }

    pub(crate) fn close_picker(&mut self) {
        self.set_picker(None);
    }

    /// Set the picker, and say so when it is not what it was: the web reads the state again only after an event.
    pub(crate) fn set_picker(&mut self, picker: Option<Picker>) {
        if self.state.picker != picker {
            self.state.picker = picker;
            self.events.push(Event::PickerChanged {});
        }
    }

    /// Bring the open picker in line with the document, the modes and the selection; close it when its node is no
    /// longer the one selected. Nothing is built while it is closed.
    pub(crate) fn refresh_picker(&mut self) {
        let Some(p) = &self.state.picker else { return };
        if !is_open_valid(self.nodes(), &self.state.selection, &p.id, p.field) {
            self.set_picker(None);
            return;
        }
        let (id, field, query) = (p.id.clone(), p.field, p.query.clone());
        let view = self.picker_view(id, field, query);
        self.panel.picker_builds += 1;
        self.set_picker(Some(view));
    }

    fn picker_view(&self, id: String, field: BindField, query: String) -> Picker {
        picker_view(self.vars(), &self.choice(), self.nodes(), id, field, query)
    }
}
