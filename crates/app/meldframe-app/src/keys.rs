//! A key press as an action (audit finding T7a, issue 324, decision 43 clause Q1): what the key makes comes from the key table
//! (`meldframe_app_rules::keys`), and the action it makes is dispatched like any other, so a key and the button that does the
//! same thing go through one path, and `Event::KeyHandled` says the key was the editor's, so a front end keeps it from the
//! browser in the same dispatch.

use crate::app::App;
use crate::effect::Effect;
use crate::event::Event;
use meldframe_app_model::{Action, KeyInput, OverlayOp, PointerInput, TextEditOp};
use meldframe_app_parts::text_edit::selected_text;
use meldframe_app_rules::keys::{Makes, decide};

/// When a key that made an action was the editor's.
enum Used {
    /// Whenever it made the action.
    Always,
    /// Only when the action changed the document: an arrow key that moves nothing goes on to whatever else answers it.
    IfDocumentChanged,
}

impl App {
    /// Take a key press: decide what it makes, do it, and say it was used when it was an editor key that had something to act on.
    pub(crate) fn key(&mut self, input: KeyInput) -> Vec<Effect> {
        let Some((action, used)) = decide(&input).and_then(|makes| self.key_action(makes)) else {
            return Vec::new();
        };
        let before = self.events.len();
        let fx = self.dispatch(action);
        let handled = match used {
            Used::Always => true,
            Used::IfDocumentChanged => self.events[before..]
                .iter()
                .any(|e| matches!(e, Event::DocumentChanged { .. })),
        };
        if handled {
            self.events.push(Event::KeyHandled {});
        }
        fx
    }

    /// The action a key makes in the state the app is in; none when there is nothing for it to act on (Delete with no selection,
    /// Enter with no one text selected, Escape with nothing open).
    fn key_action(&self, makes: Makes) -> Option<(Action, Used)> {
        let s = &self.state;
        let action = match makes {
            Makes::SetTool(tool) => Action::SetTool { tool },
            Makes::Undo => Action::Undo {},
            Makes::Redo => Action::Redo {},
            Makes::TogglePanel(panel) => Action::TogglePanel { panel },
            Makes::ToggleOverlay(overlay) => Action::Overlay {
                op: OverlayOp::Toggle { overlay },
            },
            Makes::Zoom(command) => Action::zoom(command),
            Makes::DeleteSelection if !s.selection.is_empty() => Action::Delete {
                ids: s.selection.clone(),
            },
            // With no selection the action is a no-op (no step, no event, no effect): the key raises nothing and the browser
            // keeps it.
            // An arrow key is not a command while a press is down: the gesture is the person's hands' business.
            Makes::Nudge(..) if self.pointer.busy() => return None,
            Makes::Nudge(direction, coarse) => {
                return Some((Action::Nudge { direction, coarse }, Used::IfDocumentChanged));
            }
            Makes::BeginTextEdit if selected_text(self.replica.nodes(), &s.selection).is_some() => {
                Action::TextEdit {
                    op: TextEditOp::BeginSelected {},
                }
            }
            // Escape abandons a press that is down, begun to drag or not; else it cancels the text being edited; when none is open, it closes the overlay that is.
            Makes::Cancel if self.pointer.busy() => Action::Pointer {
                input: PointerInput::cancel(),
            },
            Makes::Cancel if s.text_edit.is_some() => Action::TextEdit {
                op: TextEditOp::Cancel {},
            },
            Makes::Cancel if s.overlay.is_some() => Action::Overlay {
                op: OverlayOp::Close {
                    overlay: s.overlay?,
                },
            },
            Makes::DeleteSelection | Makes::BeginTextEdit | Makes::Cancel | Makes::Host => {
                return None;
            }
        };
        Some((action, Used::Always))
    }
}
