//! What a front end draws while a gesture goes on, as data: the answer of `App::pointer_preview`.

use meldframe_app_model::Point;
use meldframe_app_rules::drop::Slot;
use meldframe_app_rules::multi::Preview;
use serde::Serialize;

/// What the gesture that is going on would do if it ended now. The file changes only when it ends.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PointerPreview {
    /// A drag of nodes. `follow` are the nodes drawn moving with the pointer by `(dx, dy)` (world units, since the press);
    /// `slot` is the place in a flow the flow children would land in, drawn as a line (the insertion line), and their own
    /// boxes stay where they are drawn.
    #[serde(rename_all = "camelCase")]
    Move {
        follow: Vec<String>,
        slot: Option<Slot>,
        dx: f64,
        dy: f64,
    },
}

/// The preview of a drag of `ids` by `by`, from the answer of the drop rule (`App::drop_preview_many`): with no answer (the layout
/// is not on the canvas) every dragged node follows the pointer, and where the answer holds no slot only the nodes it names do.
pub fn move_preview(ids: &[String], by: Point, drop: Option<Preview>) -> PointerPreview {
    let (follow, slot) = match drop {
        None => (ids.to_vec(), None),
        Some(p) => (p.follow, p.slot),
    };
    PointerPreview::Move {
        follow,
        slot,
        dx: by.x,
        dy: by.y,
    }
}
