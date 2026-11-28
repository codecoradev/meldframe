//! `Action::Pointer`: the machine of `pointer_machine` decides, and what it decides is dispatched like any other action.

use crate::pointer_machine::{Env, PointerPreview, Queries, preview, top_level};
use crate::{app::App, effect::Effect, event::Event};
use meldframe_app_model::{Modifiers, PointerInput};

impl App {
    pub(crate) fn pointer(&mut self, input: PointerInput) -> Vec<Effect> {
        let mut machine = std::mem::take(&mut self.pointer);
        let (hit, rect) = (|x, y| self.hit(x, y), || self.selection_rect());
        let within = |r| self.on_index(|i| top_level(self.replica.nodes(), i.overlapping(r)));
        let (s, halted) = (&self.state, self.halted());
        let env = Env {
            tool: s.tool,
            camera: s.camera,
            selection: &s.selection,
            editing: s.text_edit.is_some(),
            halted,
            hit: &hit,
            selection_rect: &rect,
            within: &within,
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
    /// release; none while nothing is going on. Reads and changes nothing. Cost: that of the query it asks
    /// (`drop_preview_many`, `resize_preview`, `draw_preview`); a marquee is the rectangle alone. Modifiers change no preview.
    pub fn pointer_preview(&self) -> Option<PointerPreview> {
        let m = Modifiers::default();
        let q = Queries {
            drop: &|ids, at, by| self.drop_preview_many(ids, (at.x, at.y), (by.x, by.y)),
            resize: &|id, h, by| self.resize_preview(id, h, by, m),
            draw: &|kind, from, to| self.draw_preview(kind, from, to, m),
        };
        preview(&self.state.pointer.gesture, &q)
    }
}
