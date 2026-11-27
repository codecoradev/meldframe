//! The command of a resize in a laid-out file: the size, and the side that becomes fixed. Pure, like `drop`.

use meldframe_layout::{Rect, is_layout_frame};
use meldframe_protocol::{Command, Node, NodePatch, SizeMode};

/// A size within this many px of the drawn one is the same size (a box typed or dragged back to where it was).
const SAME: f64 = 1e-6;

/// The command that sizes `n` to `(w, h)`, and moves it to `(x, y)` when one is given.
///
/// `drawn` is the box the canvas draws it at and `parent` its parent, or `None` for a file whose canvas draws the stored
/// boxes (the host flag `layout_on_canvas` off): then the command is the plain one, the size and the origin. With a
/// drawn box, a side sized hug or fill that gets another size than it is drawn at becomes fixed in the same command (a
/// hug or fill side cannot hold a size), and the origin of a child of an auto layout frame is not written: the flow
/// decides where it is and its stored `x` and `y` stay as they were.
pub fn resize(
    n: &Node,
    drawn: Option<Rect>,
    parent: Option<&Node>,
    (w, h): (f64, f64),
    (x, y): (Option<f64>, Option<f64>),
) -> Command {
    let mut patch = NodePatch {
        x,
        y,
        w: Some(w),
        h: Some(h),
        ..Default::default()
    };
    if let Some(r) = drawn {
        if n.sw != SizeMode::Fixed && (w - r.w).abs() > SAME {
            patch.sw = Some(SizeMode::Fixed);
        }
        if n.sh != SizeMode::Fixed && (h - r.h).abs() > SAME {
            patch.sh = Some(SizeMode::Fixed);
        }
        if origin_locked(n, drawn, parent) {
            (patch.x, patch.y) = (None, None);
        }
    }
    Command::UpdateNode {
        id: n.id.clone(),
        patch,
    }
}

/// Whether the position of `n` is the layout's, so that a resize writes none: the canvas draws it from the layout (`drawn` is
/// its box) and it is a flow child, not an absolute one, of an auto layout frame. It is `Properties::position_locked`.
pub fn origin_locked(n: &Node, drawn: Option<Rect>, parent: Option<&Node>) -> bool {
    drawn.is_some() && !n.absolute && parent.is_some_and(is_layout_frame)
}
