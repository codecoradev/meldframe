//! Interaction on a laid-out canvas: a drop and a resize worked out from the boxes the canvas draws, and the box the
//! properties panel shows. The rules are `meldframe-app-rules` (`drop`, `resize`); this applies what they decide as one
//! history step, through the one replica (and so the one gate), and reads the boxes from the host's layout.
//!
//! With the host flag `layout_on_canvas` off, none of the layout rules applies: a drop is a move by the drag, a resize
//! writes the size and the origin, and the panel shows the stored box.

use crate::app::App;
use meldframe_app_model::Direction;
use meldframe_app_nudge::{can_nudge, nudge};
use meldframe_app_parts::document::{apply_all, move_by, move_to};
use meldframe_app_parts::panel::selected_node;
use meldframe_app_parts::properties::Drawn;
use meldframe_app_rules::arrow::arrow_commands;
use meldframe_app_rules::drop::{Drag, Slot, drop_commands, drop_slot};
use meldframe_app_rules::multi::{
    DragMany, Preview, drop_many_commands, drop_many_preview, top_nodes,
};
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

    /// A selection was dropped (`Action::DropNodes`): one step, or none when nothing changes. With the flag on the rules of a
    /// block of flow children decide (`meldframe-app-rules`, `multi`); with it off the move is the one a drag always was.
    pub(crate) fn drop_nodes(
        &mut self,
        ids: &[String],
        at: (f64, f64),
        by: (f64, f64),
    ) -> Result<(), SubmitError> {
        if self.canvas.enabled {
            let nodes = self.replica.nodes();
            let cmds = self
                .with_rects(|rects| drop_many_commands(nodes, rects, &DragMany { ids, at, by }))?;
            self.validate(&cmds)?;
            return apply_all(&mut self.replica, &cmds);
        }
        self.move_dragged(ids, at, by)
    }

    /// The move by the drag that the layout does not touch: one node is the plain move of `drop_node`, the selection in one
    /// step when `ids` are the selection and none is under another, else each top node on its own step (as the web sent
    /// them before the layout rules).
    fn move_dragged(
        &mut self,
        ids: &[String],
        at: (f64, f64),
        by: (f64, f64),
    ) -> Result<(), SubmitError> {
        let nodes = self.replica.nodes();
        let tops: Vec<String> = top_nodes(nodes, ids)?
            .iter()
            .map(|n| n.id.clone())
            .collect();
        if let [one] = &tops[..] {
            let cmds = drop_commands(nodes, &[], &Drag { id: one, at, by })?;
            self.validate(&cmds)?;
            return apply_all(&mut self.replica, &cmds);
        }
        let selection = &self.state.selection;
        let whole = tops.len() == ids.len()
            && ids.len() == selection.len()
            && ids.iter().all(|i| selection.contains(i));
        if whole {
            let cmds = move_by(selection, nodes, by.0, by.1);
            self.validate(&cmds)?;
            return apply_all(&mut self.replica, &cmds);
        }
        for id in &tops {
            let Some(n) = self.replica.nodes().iter().find(|n| n.id == *id) else {
                continue;
            };
            let cmd = move_to(id, n.x + by.0, n.y + by.1);
            self.replica.apply(&cmd)?;
        }
        Ok(())
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

    /// `Action::Nudge`: one press of an arrow key as one step (`meldframe-app-nudge`); none when nothing moves.
    pub(crate) fn nudge(&mut self, key: Direction, coarse: bool) -> Result<(), SubmitError> {
        nudge(
            &mut self.replica,
            &self.state.selection,
            (key, coarse, self.canvas.enabled),
        )
    }

    /// Whether that press would move anything and be taken: what a button that sends it is on for.
    pub fn can_nudge(&self, key: Direction, coarse: bool) -> bool {
        can_nudge(
            &self.replica,
            &self.state.selection,
            (key, coarse, self.canvas.enabled),
        )
    }

    /// Where in a flow a drop at `at` after a drag of `by` would put the node (what a front end draws while the pointer is
    /// down), from the same decision as the drop. None with the flag off and for a drop that goes into no flow. It reads
    /// and changes nothing.
    ///
    /// Cost: the file is laid out once for a revision of the document and a measurer, and the app keeps the boxes, so the
    /// calls that follow while the pointer moves and nothing else changes read them (`layout_runs` counts the layouts).
    /// What is left on each call is the rule itself, a few passes over the nodes. When the pointer is not inside the
    /// node's own flow parent the rule builds a `HitIndex` (`meldframe_render`: a hash map and a vector over the boxes) on
    /// each call. Under WASM (Node, release, a column of 5,000 children) it was about 2.6 ms a call with a layout on every
    /// call and about 1.0 ms with the cache (0.46 and 0.17 ms for 1,000 nodes); that was measured before the rule read the
    /// hit index and has not been measured again under WASM. Natively (release, 5,000 boxes) the index costs 0.45 ms for
    /// `HitIndex::new` plus one query, against 0.10 ms for the by-id map the rule built before.
    pub fn drop_preview(&self, id: &str, at: (f64, f64), by: (f64, f64)) -> Option<Slot> {
        if !self.canvas.enabled {
            return None;
        }
        let nodes = self.replica.nodes();
        self.with_rects(|rects| drop_slot(nodes, rects, &Drag { id, at, by }))
    }

    /// What a front end shows while a selection is dragged (`Action::DropNodes`): the slot the flow children would land in
    /// and which of the dragged nodes follow the pointer, from the same decision as the drop. None with the flag off (the
    /// nodes then simply follow the pointer), when none of the ids is a node and for a number that is not one. A drag of one
    /// node is [`App::drop_preview`] with the node as `follow` when there is no slot. It reads and changes nothing. Cost: the
    /// layout once per revision of the document and measurer (see `drop_preview`), and on each call a few passes over the
    /// nodes and the selection, linear in both (a test holds 5,000 selected siblings to a budget).
    pub fn drop_preview_many(
        &self,
        ids: &[String],
        at: (f64, f64),
        by: (f64, f64),
    ) -> Option<Preview> {
        if !self.canvas.enabled {
            return None;
        }
        let nodes = self.replica.nodes();
        self.with_rects(|rects| drop_many_preview(nodes, rects, &DragMany { ids, at, by }))
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

    /// The box the layout gives `id`, none with the flag off or for a node the layout holds no box of: what `resize` reads.
    pub(crate) fn layout_box(&self, id: &str) -> Option<Rect> {
        self.with_rects(|rects| rects.iter().find(|(i, _)| i == id).map(|(_, r)| *r))
    }

    /// `Action::Resize`: the size, the origin, and with laid-out boxes the side that becomes fixed.
    pub(crate) fn resize_node(
        &mut self,
        id: &str,
        size: (f64, f64),
        origin: (Option<f64>, Option<f64>),
    ) -> Result<(), SubmitError> {
        let cmd = self.resize_command(id, size, origin)?;
        self.replica.apply(&cmd)
    }

    /// The command `Action::Resize` makes, worked out and not applied (a preview checks it as the release would be).
    pub(crate) fn resize_command(
        &self,
        id: &str,
        size: (f64, f64),
        origin: (Option<f64>, Option<f64>),
    ) -> Result<meldframe_protocol::Command, ApplyError> {
        let nodes = self.replica.nodes();
        let Some(n) = nodes.iter().find(|n| n.id == id) else {
            return Err(ApplyError::NotFound(id.to_string()));
        };
        let drawn = self.layout_box(id);
        let parent = n
            .parent_id
            .as_deref()
            .and_then(|p| nodes.iter().find(|x| x.id == p));
        Ok(resize(n, drawn, parent, size, origin))
    }

    /// The box the canvas draws the one selected node at, when the canvas reads laid-out boxes (the panel shows it).
    pub(crate) fn drawn_selection(&self) -> Option<Drawn> {
        if !self.canvas.enabled {
            return None;
        }
        let id = &selected_node(&self.state.selection, self.replica.nodes())?.id;
        self.with_scene(|_, boxes| {
            boxes.iter().find(|b| b.id == *id).map(|b| Drawn {
                x: b.x,
                y: b.y,
                w: b.w,
                h: b.h,
            })
        })
    }
}
