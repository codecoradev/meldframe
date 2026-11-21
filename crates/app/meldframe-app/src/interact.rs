//! Interaction on a laid-out canvas: a drop and a resize worked out from the boxes the canvas draws, and the box the
//! properties panel shows. The rules are `meldframe-app-rules` (`drop`, `resize`); this applies what they decide as one
//! history step, through the one replica (and so the one gate), and reads the boxes from the host's layout.
//!
//! With the host flag `layout_on_canvas` off, none of the layout rules applies: a drop is a move by the drag, a resize
//! writes the size and the origin, and the panel shows the stored box.

use crate::app::App;
use meldframe_app_parts::document::apply_all;
use meldframe_app_parts::panel::selected_node;
use meldframe_app_parts::properties::Drawn;
use meldframe_app_rules::drop::{Drag, drop_commands};
use meldframe_app_rules::resize::resize;
use meldframe_check::ApplyError;
use meldframe_sync::SubmitError;

impl App {
    /// A node was dropped with the pointer at `at` after a drag of `by` (`Action::DropNode`): one step, or none when
    /// it stays where it is.
    pub(crate) fn drop_node(
        &mut self,
        id: &str,
        at: (f64, f64),
        by: (f64, f64),
    ) -> Result<(), SubmitError> {
        let nodes = self.replica.nodes();
        // Without the laid-out boxes the rule has none to read, and a drop is a move by the drag.
        let rects = if self.canvas.enabled {
            self.canvas.rects(nodes)
        } else {
            Vec::new()
        };
        let cmds = drop_commands(nodes, &rects, &Drag { id, at, by })?;
        self.validate(&cmds)?;
        apply_all(&mut self.replica, &cmds)
    }

    /// `Action::Resize`: the size, the origin, and with laid-out boxes the side that becomes fixed.
    pub(crate) fn resize_node(
        &mut self,
        id: &str,
        size: (f64, f64),
        origin: (Option<f64>, Option<f64>),
    ) -> Result<(), SubmitError> {
        let nodes = self.replica.nodes();
        let Some(n) = nodes.iter().find(|n| n.id == id) else {
            return Err(ApplyError::NotFound(id.to_string()).into());
        };
        let rects = if self.canvas.enabled {
            self.canvas.rects(nodes)
        } else {
            Vec::new()
        };
        let drawn = rects.iter().find(|(i, _)| i == id).map(|(_, r)| *r);
        let parent = n
            .parent_id
            .as_deref()
            .and_then(|p| nodes.iter().find(|x| x.id == p));
        self.replica.apply(&resize(n, drawn, parent, size, origin))
    }

    /// The box the canvas draws the one selected node at, when the canvas reads laid-out boxes (the panel shows it).
    pub(crate) fn drawn_selection(&self) -> Option<Drawn> {
        if !self.canvas.enabled {
            return None;
        }
        let id = &selected_node(&self.state.selection, self.replica.nodes())?.id;
        self.scene()
            .into_iter()
            .find(|b| b.id == *id)
            .map(|b| Drawn {
                x: b.x,
                y: b.y,
                w: b.w,
                h: b.h,
            })
    }
}
