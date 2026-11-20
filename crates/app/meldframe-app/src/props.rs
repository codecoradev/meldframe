//! The values of the properties panel: each `Property` becomes one `UpdateNode` (one undo step). The rules are the
//! pure ones of `meldframe-app-parts` (`property`); this applies the patch they make.

use crate::action::Property;
use crate::app::App;
use crate::document::refuse;
use meldframe_app_parts::document::update;
use meldframe_app_parts::property::property_patch;
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
}
