//! The camera of the canvas (E8.1.4, part 1): its data (`Camera`, `Viewport`, the zoom commands and `CameraOp`, the action a
//! front end sends), the rules that work on it (the zoom limits, the mapping between the world and the screen, pan, zoom around
//! a point, fit, the meaning of a wheel and a pinch, the rectangles a fit looks at), and the two small inputs they are made of
//! (a point and the modifier keys held). Pure: no state of the app, no clock, no I/O. `meldframe-app-model` holds the camera in
//! `AppState` and sends it in `Action::Camera`, `meldframe-app` applies the ops. A crate of its own so that neither of those
//! passes the size at which a crate has to be split into modules.

mod input;
mod rules;
mod types;

pub use input::{Modifiers, Point};
pub use rules::*;
pub use types::{Camera, CameraOp, Viewport, Wheel, WheelUnit, ZoomCommand};
