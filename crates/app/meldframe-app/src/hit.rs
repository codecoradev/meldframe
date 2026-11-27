//! What is under a point of the canvas (T9 of the thin UI audit, decision 43): the query a front end asks on a press, for
//! the tool that creates and for the hover. The rule is `meldframe_render::HitIndex`, the same one a drop reads; this keeps
//! the index of the scene for the revision of the document (`crate::canvas`) and answers from it.

use crate::app::App;

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
        self.canvas.with_hits(
            self.replica.revision(),
            || self.scene(),
            |index| index.front_most(x, y).map(str::to_owned),
        )
    }

    /// How many times the app built the index of the scene to answer `hit` (a second question about a file that has not
    /// changed, with the same measurer and flag, costs none).
    pub fn hit_builds(&self) -> u64 {
        self.canvas.hit_builds()
    }
}
