//! Interaction on a laid-out canvas: a drop and a resize worked out from the boxes the canvas draws, and the box the
//! properties panel shows. The rules are `meldframe-app-rules` (`drop`, `resize`); this applies what they decide as one
//! history step, through the one replica (and so the one gate), and reads the boxes from the host's layout.
//!
//! With the host flag `layout_on_canvas` off, none of the layout rules applies: a drop is a move by the drag, a resize
//! writes the size and the origin, and the panel shows the stored box.

use crate::app::App;
use meldframe_app_model::Direction;
use meldframe_app_parts::document::apply_all;
use meldframe_app_parts::panel::selected_node;
use meldframe_app_parts::properties::Drawn;
use meldframe_app_rules::arrow::arrow_commands;
use meldframe_app_rules::drop::{Drag, Slot, drop_commands, drop_slot};
use meldframe_app_rules::resize::resize;
use meldframe_check::ApplyError;
use meldframe_layout::Rect;
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
        let cmds = self.with_rects(|rects| drop_commands(nodes, rects, &Drag { id, at, by }))?;
        self.validate(&cmds)?;
        apply_all(&mut self.replica, &cmds)
    }

    /// An arrow key with the selection (`Action::ArrowKey`): the selected children of an auto layout frame move one place
    /// along its flow, as one step (`meldframe-app-rules`, `arrow`). With the flag off it does nothing.
    pub(crate) fn arrow_key(&mut self, key: Direction) -> Result<(), SubmitError> {
        if !self.canvas.enabled {
            return Ok(());
        }
        let cmds = arrow_commands(self.replica.nodes(), &self.state.selection, key);
        self.validate(&cmds)?;
        apply_all(&mut self.replica, &cmds)
    }

    /// Where in a flow a drop at `at` after a drag of `by` would put the node (what a front end draws while the pointer is
    /// down), from the same decision as the drop. None with the flag off and for a drop that goes into no flow. It reads
    /// and changes nothing.
    ///
    /// Cost: the file is laid out once for a revision of the document and a measurer, and the app keeps the boxes, so the
    /// calls that follow while the pointer moves and nothing else changes read them (`layout_runs` counts the layouts).
    /// What is left on each call is the rule itself, a few passes over the nodes. Under WASM (Node, release, a column of
    /// 5,000 children): about 2.6 ms a call with a layout on every call, about 1.0 ms with the cache (0.46 and 0.17 ms for
    /// 1,000 nodes).
    pub fn drop_preview(&self, id: &str, at: (f64, f64), by: (f64, f64)) -> Option<Slot> {
        if !self.canvas.enabled {
            return None;
        }
        let nodes = self.replica.nodes();
        self.with_rects(|rects| drop_slot(nodes, rects, &Drag { id, at, by }))
    }

    /// The laid-out boxes for a rule to read, kept for the revision of the document and the measurer (empty with the flag
    /// off: the rules then have none to read and fall back to the stored boxes).
    fn with_rects<R>(&self, read: impl FnOnce(&[(String, Rect)]) -> R) -> R {
        if !self.canvas.enabled {
            return read(&[]);
        }
        self.canvas
            .with_rects(self.replica.revision(), self.replica.nodes(), read)
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
        let drawn = self.with_rects(|rects| rects.iter().find(|(i, _)| i == id).map(|(_, r)| *r));
        let parent = n
            .parent_id
            .as_deref()
            .and_then(|p| nodes.iter().find(|x| x.id == p));
        let cmd = resize(n, drawn, parent, size, origin);
        self.replica.apply(&cmd)
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
