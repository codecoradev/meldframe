//! The Layers panel: its rows are built here, once per dispatch, with every name made safe to show (rule U9), so a
//! front end draws `AppState::layers` and reads no name from the file.

use crate::app::App;
use meldframe_app_parts::layers::layer_rows;

impl App {
    /// Bring `AppState::layers` in line with the file, the collapsed ids, the selection and the last actors. Nothing
    /// is listed while the panel is closed.
    pub(crate) fn refresh_layers(&mut self) {
        if !self.state.panels.layers {
            self.state.layers.clear();
            return;
        }
        let s = &self.state;
        let rows = layer_rows(
            self.replica.nodes(),
            &s.panels.collapsed,
            &s.selection,
            &s.last_actor,
        );
        if self.state.layers != rows {
            self.state.layers = rows;
        }
    }
}
