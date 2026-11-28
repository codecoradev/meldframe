//! The shortcut sheet and the command palette as state of the app (audit finding T11, issue 324): one is open at a time, and
//! the palette stays shut while a dialog is open; menus, tooltips and popovers are not here, for their open state is focus and DOM
//! behaviour, and stays with the component library (a front end does not send the palette key while one of its menus is open).

use crate::app::App;
use crate::event::Event;
use meldframe_app_model::{Overlay, OverlayOp};

impl App {
    /// Open, close or toggle an overlay.
    pub(crate) fn overlay_action(&mut self, op: OverlayOp) {
        let overlay = op.overlay();
        let is_open = self.state.overlay == Some(overlay);
        match op {
            // Only the overlay that is open can be closed: a stale close of the other one changes nothing.
            OverlayOp::Close { .. } if !is_open => {}
            OverlayOp::Close { .. } | OverlayOp::Toggle { .. } if is_open => self.set_overlay(None),
            // An open of what is open changes nothing (`set_overlay` says nothing then).
            _ if overlay == Overlay::Palette && self.dialog_open() => {}
            _ => self.set_overlay(Some(overlay)),
        }
    }

    /// Whether a dialog is open: Export, Export variables, Import variables, the manage view, or the bind picker (a popover
    /// whose open state the app holds, and that a page draws as a dialog).
    fn dialog_open(&self) -> bool {
        let s = &self.state;
        s.export.dialog.is_some()
            || s.variables_io.export.is_some()
            || s.variables_io.import_open
            || s.manage.open
            || s.picker.is_some()
    }

    /// The palette is never open with a dialog: one that opens over it (from an action of another front end, or an agent; the
    /// palette closes itself first when a command of its own opens a dialog) closes it. The sheet may stay beside a dialog.
    /// Run once per dispatch, so it holds whichever action opened the dialog.
    pub(crate) fn settle_overlay(&mut self) {
        if self.state.overlay == Some(Overlay::Palette) && self.dialog_open() {
            self.set_overlay(None);
        }
    }

    fn set_overlay(&mut self, overlay: Option<Overlay>) {
        if self.state.overlay != overlay {
            self.state.overlay = overlay;
            self.events.push(Event::OverlayChanged { overlay });
        }
    }
}
