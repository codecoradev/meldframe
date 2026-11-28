//! The values of the properties panel: each `Property` becomes one `UpdateNode` (one undo step); the rules are the
//! pure ones of `meldframe-app-parts` (`property`), and this applies the patch they make.

use crate::action::Property;
use crate::app::App;
use crate::document::refuse;
use crate::parts::document::{move_to, update};
use crate::parts::property::property_patch;
use crate::parts::typed::{Typed, typed};
use meldframe_app_model::TextField;
use meldframe_render::Fields;
use meldframe_sync::SubmitError;

impl App {
    pub(crate) fn set_property(&mut self, id: &str, property: Property) -> Result<(), SubmitError> {
        let Some(node) = self.nodes().iter().find(|n| n.id == id).cloned() else {
            return refuse(&mut self.events, "not_found");
        };
        let choice = self.choice();
        let painted = Fields::new(self.vars(), &choice).fill_text(&node.fill);
        match property_patch(&node, property, &painted) {
            Ok(Some(patch)) => self.replica.apply(&update(id, patch)),
            Ok(None) => Ok(()),
            Err(code) => refuse(&mut self.events, code),
        }
    }

    /// What was typed in a field of the panel of the selected node: the rules are `crate::parts::typed`, which
    /// says what it comes to; this carries it out the way the action of that kind is.
    pub(crate) fn set_property_text(
        &mut self,
        id: &str,
        field: &TextField,
        text: &str,
    ) -> Result<(), SubmitError> {
        if !self.nodes().iter().any(|n| n.id == id) {
            return refuse(&mut self.events, "not_found");
        }
        let Some(panel) = self.state.properties.clone().filter(|p| p.id == id) else {
            return refuse(&mut self.events, "not_selected");
        };
        match typed(&panel, field, text) {
            Ok(Typed::Nothing) => Ok(()),
            Ok(Typed::Property(property)) => self.set_property(id, property),
            Ok(Typed::Move { x, y }) => self.replica.apply(&move_to(id, x, y)),
            Ok(Typed::Resize { w, h }) => self.resize_node(id, (w, h), (None, None)),
            Err(code) => refuse(&mut self.events, code),
        }
    }
}
