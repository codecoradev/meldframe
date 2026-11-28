//! What is under a point of the canvas (T9 of the thin UI audit, decision 43): the query a front end asks on a press, for
//! the tool that creates and for the hover, answered by `meldframe_render::HitIndex`, the same rule a drop reads; this keeps
//! the index of the scene for the revision of the document (`crate::canvas`) and answers from it.

use crate::app::App;
use meldframe_layout::Rect;
use meldframe_render::{HitIndex, extent_of};

impl App {
    /// The id of the front-most node under the world point `(x, y)`, or none: a node that is hidden, or that a clipping
    /// ancestor cuts away at the point, is not under it, and the left and top edges of a box are in and the right and bottom
    /// ones out (`meldframe_render::HitIndex`). It reads the scene the canvas draws (`App::scene`, so the laid-out boxes with
    /// the flag on) and changes nothing, raises no event and reads no state of the editor: not the selection, not the tool.
    ///
    /// Cost: the scene is worked out and indexed once for a revision of the document, the measurer and the flag, and the
    /// calls that follow read the index (`App::hit_builds` counts the builds); what is left on a call is one pass over the
    /// boxes from the front, which stops at the first hit.
    pub fn hit(&self, x: f64, y: f64) -> Option<String> {
        self.on_index(|index| index.front_most(x, y).map(str::to_owned))
    }

    /// The box around the selected nodes in world space, none for none: what the selection box and its handles are drawn from.
    pub(crate) fn selection_rect(&self) -> Option<Rect> {
        self.on_index(|index| extent_of(index, &self.state.selection))
    }

    /// The index of the scene, kept for the revision of the document, the measurer and the flag: the one thing the rules that
    /// read where boxes are (`hit`, the box of a node, the selection box) ask.
    pub(crate) fn on_index<R>(&self, read: impl FnOnce(&HitIndex) -> R) -> R {
        self.canvas.with_hits(
            self.replica.revision(),
            || self.with_scene(|_, boxes| HitIndex::from_boxes(boxes)),
            read,
        )
    }

    /// The number that names the index `hit` reads: it goes up when the document, the measurer or the `layout_on_canvas` flag
    /// changed, which are the things a box's place depends on, and stays when only the modes picked change (a mode changes
    /// what is painted, never where a box is) and for a camera, which is no input of the scene. Asking builds nothing.
    pub fn hit_index_epoch(&self) -> u64 {
        self.canvas.hit_epoch(self.replica.revision())
    }

    /// How many times the app built the index of the scene to answer `hit` (a second question about a file that has not
    /// changed, with the same measurer and flag, costs none).
    pub fn hit_builds(&self) -> u64 {
        self.canvas.hit_builds()
    }
}
