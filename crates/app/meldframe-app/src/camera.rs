//! `Action::Camera`: pan, zoom and fit as view state (E8.1.4); the rules are `meldframe_camera`.

use crate::{app::App, document::refuse, event::Event};
use meldframe_app_model::CameraOp;
use meldframe_camera::{apply, bounds};

impl App {
    pub(crate) fn camera_op(&mut self, op: CameraOp) -> Vec<crate::effect::Effect> {
        let (c, v, sel) = (
            self.state.camera,
            self.state.viewport,
            &self.state.selection,
        );
        match apply(&op, c, v, || self.with_scene(|_, b| bounds(b, sel))) {
            None => drop(refuse(&mut self.events, "invalid")),
            Some(n) if n == (c, v) => {}
            Some(n) => {
                (self.state.camera, self.state.viewport) = n;
                self.events.push(Event::CameraChanged {});
            }
        }
        Vec::new()
    }
}
