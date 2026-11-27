//! How the canvas gets its boxes: the host's text measurer and the `layout_on_canvas` flag, as the app's methods. The state is
//! `meldframe_app_rules::canvas::CanvasLayout`.

use crate::app::App;
use crate::effect::Effect;
use meldframe_app_rules::canvas::MeasurerBox;

impl App {
    /// Whether the canvas reads laid-out boxes (see `meldframe_app_rules::canvas`). Off until the host turns it on.
    pub fn layout_on_canvas(&self) -> bool {
        self.canvas.enabled
    }

    /// Turn the laid-out scene on or off. Returns the effect the host carries out: `Effect::Render` with an empty
    /// list (everything) when the flag changed, nothing when it did not. No event is raised, so a front end that reads the
    /// scene on events must read it again itself after this call. The only state it changes is the properties of the
    /// selected node: they show the box the canvas draws (`Properties::position_locked`), so they are brought up to date; so are
    /// the plans of the PNG export (`AppState::export.png`), with `Event::ExportChanged` when one changed.
    pub fn set_layout_on_canvas(&mut self, on: bool) -> Vec<Effect> {
        if self.canvas.enabled == on {
            return Vec::new();
        }
        self.canvas.enabled = on;
        self.refresh_properties();
        // The plan of the PNG export reads the scene, so it moves with the flag (`ExportChanged` when it did).
        self.refresh_png();
        redraw()
    }

    /// How many times the app laid the whole file out to answer a drop, a preview or a resize (the scene is worked out
    /// apart from this). A second question about a file that has not changed, with the same measurer, costs none.
    pub fn layout_runs(&self) -> u64 {
        self.canvas.runs()
    }

    /// Replace the text measurer (the default is `StubMeasurer`). The canvas lays text out with it while the flag is on,
    /// so the redraw is asked for only then. The Export dialog measures with it whatever the flag is, so a hug text exports
    /// the width the canvas draws: an open dialog is built again (`Event::ExportChanged` when it changed), and nothing is
    /// built while it is closed. No measurer given, the stub measures, as it always did.
    pub fn set_measurer(&mut self, measurer: MeasurerBox) -> Vec<Effect> {
        self.canvas.set_measurer(measurer);
        self.measurer_changed();
        if self.canvas.enabled {
            self.refresh_properties();
            redraw()
        } else {
            Vec::new()
        }
    }
}

fn redraw() -> Vec<Effect> {
    vec![Effect::Render { nodes: Vec::new() }]
}
