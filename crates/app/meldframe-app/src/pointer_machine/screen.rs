//! What the machine reads off the screen and the file: a point in the world, the travel since a press, the handle of the selection
//! box under a point, the cursor of a handle and the node kind of a tool.

use crate::pointer_machine::Env;
use meldframe_app_model::camera::{rect_to_screen, to_world};
use meldframe_app_model::{CreateKind, Handle, Point, PointerCursor, Tool};
use meldframe_app_rules::canvas_overlay::handle_at;

/// A point on the screen in CSS pixels from the top left corner of the canvas.
pub(crate) type Screen = (f64, f64);

pub(crate) fn world(env: &Env, (x, y): Screen) -> Option<Point> {
    let p = to_world(&env.camera, Point { x, y });
    (p.x.is_finite() && p.y.is_finite()).then_some(p)
}

pub(crate) fn dist(a: Screen, b: Screen) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// The travel in the world from the point of the press to where the pointer is now.
pub(crate) fn travel(from: Point, at: Point) -> Point {
    Point {
        x: at.x - from.x,
        y: at.y - from.y,
    }
}

/// The handle of the selection box at a screen point, none for no selection.
pub(crate) fn handle_under(env: &Env, (x, y): Screen) -> Option<Handle> {
    if env.selection.is_empty() {
        return None;
    }
    let b = rect_to_screen(&env.camera, &(env.selection_rect)()?);
    handle_at(&b, x, y)
}

pub(crate) fn resize_cursor(h: Handle) -> PointerCursor {
    match h {
        Handle::Nw | Handle::Se => PointerCursor::NwseResize,
        Handle::Ne | Handle::Sw => PointerCursor::NeswResize,
        Handle::N | Handle::S => PointerCursor::NsResize,
        Handle::E | Handle::W => PointerCursor::EwResize,
    }
}

pub(crate) fn kind_of(tool: Tool) -> Option<CreateKind> {
    match tool {
        Tool::Select => None,
        Tool::Frame => Some(CreateKind::Frame),
        Tool::Rectangle => Some(CreateKind::Rectangle),
        Tool::Ellipse => Some(CreateKind::Ellipse),
        Tool::Text => Some(CreateKind::Text),
    }
}
