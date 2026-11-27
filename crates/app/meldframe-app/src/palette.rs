//! The run palette through the app: the entries for the state and a query, and running one by its id.

use crate::app::App;
use crate::effect::Effect;
use meldframe_app_model::{Action, Overlay, OverlayOp};
use meldframe_app_rules::palette::{Entry, Plan, Zoom, entries, filter, plan};

/// What running an entry came to.
#[derive(Debug)]
pub enum Ran {
    /// The app did it; these are the effects to carry out.
    Done(Vec<Effect>),
    /// The camera is the front end's: it does this zoom itself.
    Host(Zoom),
    /// No such entry, or it cannot run in this state: nothing changed.
    Refused,
}

impl App {
    /// The entries a query leaves for the state of the editor, best match first. `mac` is how the platform writes a key. The entry
    /// "Auto layout on canvas" is always there through the app, which owns the switch; the case of a page without it exists only at
    /// `entries` (`None`).
    pub fn palette(&self, query: &str, mac: bool) -> Vec<Entry> {
        filter(
            &entries(&self.state, mac, Some(self.layout_on_canvas())),
            query,
        )
    }

    /// Run the entry `id`. The palette closes first, so a dialog that the entry opens is not closed behind it. `centre` is the world
    /// point at the middle of the view, where a new frame is centred; a point that is not a finite number refuses the entry before
    /// anything happens. Turning "Auto layout on canvas" on or off sets the host flag and nothing else: remembering the choice and
    /// the notice the web front end shows (`toggleWithNotice`) stay with the host.
    pub fn run_palette(&mut self, id: &str, centre: (f64, f64)) -> Ran {
        if !(centre.0.is_finite() && centre.1.is_finite()) {
            return Ran::Refused;
        }
        let on = self.layout_on_canvas();
        let Some(e) = entries(&self.state, false, Some(on))
            .into_iter()
            .find(|e| e.id == id && e.enabled)
        else {
            return Ran::Refused;
        };
        let mut fx = self.dispatch(Action::Overlay {
            op: OverlayOp::Close {
                overlay: Overlay::Palette,
            },
        });
        match plan(e.run, &self.state, on, centre) {
            Plan::Dispatch(action) => fx.extend(self.dispatch(action)),
            Plan::SetLayoutOnCanvas(on) => fx.extend(self.set_layout_on_canvas(on)),
            Plan::Host(zoom) => return Ran::Host(zoom),
            Plan::Nothing => {}
        }
        Ran::Done(fx)
    }
}
