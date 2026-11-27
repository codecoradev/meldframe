//! `Action::Pointer`: the machine of `meldframe_app_pointer` decides, and what it decides is dispatched like any other action.

use crate::{app::App, effect::Effect, event::Event};
use meldframe_app_model::{PointerGesture, PointerInput};
use meldframe_app_pointer::{Env, PointerPreview, move_preview};

impl App {
    pub(crate) fn pointer(&mut self, input: PointerInput) -> Vec<Effect> {
        let mut machine = std::mem::take(&mut self.pointer);
        let (hit, rect) = (|x, y| self.hit(x, y), || self.selection_rect());
        let (s, halted) = (&self.state, self.halted());
        let env = Env {
            tool: s.tool,
            camera: s.camera,
            selection: &s.selection,
            editing: s.text_edit.is_some(),
            halted,
            hit: &hit,
            selection_rect: &rect,
        };
        let acts = machine.step(&input, &env);
        self.pointer = machine;
        self.pointer_shown();
        acts.into_iter().flat_map(|a| self.dispatch(a)).collect()
    }

    /// `AppState::pointer` is what the machine shows; `Event::PointerChanged` when that is not what it was.
    fn pointer_shown(&mut self) {
        let view = self.pointer.view();
        if *view != self.state.pointer {
            self.state.pointer = view.clone();
            self.events.push(Event::PointerChanged {});
        }
    }

    /// After an action (`from` is how many events there were before it): a new tool or selection shows in the cursor and the hover
    /// at the last point of the pointer, and a node that goes leaves the hover and ends a drag that holds it. Another change of
    /// the file shows at the next move: the hover is not asked again for each op that comes in.
    pub(crate) fn pointer_settle(&mut self, from: usize) {
        let seen = &self.events[from..];
        let tool_or_selection = seen.iter().any(|e| {
            matches!(
                e,
                Event::ToolChanged { .. } | Event::SelectionChanged { .. }
            )
        });
        if seen
            .iter()
            .any(|e| matches!(e, Event::DocumentChanged { .. }))
        {
            let nodes = self.replica.nodes();
            self.pointer.forget(&|id| nodes.iter().any(|n| n.id == id));
        }
        match self.pointer.refresh().filter(|_| tool_or_selection) {
            Some(input) => drop(self.pointer(input)),
            None => self.pointer_shown(),
        }
    }

    /// What the gesture that is going on would do if it ended now (`AppState::pointer.gesture`), from the same decision as the
    /// release; none while nothing is going on. Reads and changes nothing. Cost: that of [`App::drop_preview_many`].
    pub fn pointer_preview(&self) -> Option<PointerPreview> {
        let PointerGesture::Move { ids, at, by } = &self.state.pointer.gesture else {
            return None;
        };
        let drop = self.drop_preview_many(ids, (at.x, at.y), (by.x, by.y));
        Some(move_preview(ids, *by, drop))
    }
}
