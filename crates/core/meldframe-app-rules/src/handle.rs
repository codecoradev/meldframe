//! The box after a handle of a node was dragged: the geometry of a resize by handle (T8 of the thin UI audit, decision 43).
//! Pure, like `resize`: the edge or corner the handle holds moves by the drag, the opposite one stays, and a side never gets
//! below [`MIN_SIZE`], so a node cannot be made too small to grab again.

use crate::resize::origin_locked;
use meldframe_app_model::Handle;
use meldframe_check::ApplyError;
use meldframe_layout::Rect;
use meldframe_protocol::Node;
use serde::Serialize;

/// The smallest size a resize or a drawn box leaves a side (a node of no size cannot be grabbed again).
pub const MIN_SIZE: f64 = 1.0;

/// The box `b` after `handle` moved by `(dx, dy)` (world units, which are the units of the box's own space too: a space is
/// only offset, never scaled). Every number is finite, and a side the handle holds and moves is at least [`MIN_SIZE`] in the
/// answer. A side it does not hold, or holds with no travel, is as it was, whatever its size (a frame that hugs nothing is
/// drawn 0 wide, and dragging its south handle leaves the width alone). Refused as `NotFinite` for a number that is not one, and for a drag so long that the box would
/// not be a number.
pub fn resized_box(b: Rect, handle: Handle, dx: f64, dy: f64) -> Result<Rect, ApplyError> {
    if ![b.x, b.y, b.w, b.h].iter().all(|v| v.is_finite()) {
        return Err(ApplyError::NotFinite("box"));
    }
    if !(dx.is_finite() && dy.is_finite()) {
        return Err(ApplyError::NotFinite("dx or dy"));
    }
    let (west, east, north, south) = handle.edges();
    let Rect {
        mut x,
        mut y,
        mut w,
        mut h,
    } = b;
    if east {
        w = b.w + dx;
    }
    if west {
        x = b.x + dx;
        w = b.w - dx;
    }
    if south {
        h = b.h + dy;
    }
    if north {
        y = b.y + dy;
        h = b.h - dy;
    }
    // Only a side the handle holds, and moves, is kept from getting too small: a side it does not touch, or touches by
    // nothing, stays as it is drawn (a hug frame that is drawn 0 wide, a text 0.5 wide), so a drag that moves nothing
    // changes nothing.
    if (west || east) && dx != 0.0 && w < MIN_SIZE {
        if west {
            x = b.x + b.w - MIN_SIZE;
        }
        w = MIN_SIZE;
    }
    if (north || south) && dy != 0.0 && h < MIN_SIZE {
        if north {
            y = b.y + b.h - MIN_SIZE;
        }
        h = MIN_SIZE;
    }
    if ![x, y, w, h].iter().all(|v| v.is_finite()) {
        return Err(ApplyError::NotFinite("size"));
    }
    Ok(Rect { x, y, w, h })
}

/// What a handle drag shows while it goes on: the box of the node after the drag, in its parent's space and in world space.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResizePreview {
    pub id: String,
    /// The box in the space of the node's parent (the fields `Action::Resize` writes).
    pub local: Rect,
    /// The same box on the canvas.
    pub world: Rect,
    /// The position is the layout's (a child of an auto layout frame, with the layout on the canvas): the box keeps the
    /// origin it is drawn at and only its size changes, because the resize writes no position and the flow places the node.
    pub origin_fixed: bool,
}

/// What a drag of `handle` by `(dx, dy)` does to a node that is drawn at `from` in the space of its parent, whose origin on
/// the canvas is `origin`. `drawn` is the box the layout gives the node and `parent` its parent, as `resize` reads them.
/// The answer is the box the drag leaves, which equals `from` when it changes nothing.
pub fn resize_plan(
    n: &Node,
    from: Rect,
    origin: (f64, f64),
    (drawn, parent): (Option<Rect>, Option<&Node>),
    (handle, dx, dy): (Handle, f64, f64),
) -> Result<ResizePreview, ApplyError> {
    let b = resized_box(from, handle, dx, dy)?;
    let fixed = origin_locked(n, drawn, parent);
    let local = if fixed {
        Rect {
            x: from.x,
            y: from.y,
            w: b.w,
            h: b.h,
        }
    } else {
        b
    };
    let world = Rect {
        x: origin.0 + local.x,
        y: origin.1 + local.y,
        ..local
    };
    Ok(ResizePreview {
        id: n.id.clone(),
        local,
        world,
        origin_fixed: fixed,
    })
}
