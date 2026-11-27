//! The two small inputs of the app layer that the camera is made of (E8.1.4): a point and the modifier keys held, with their
//! TypeScript types. `meldframe-app-model` re-exports them. A leaf crate so that the camera (its data, its action and its rules,
//! next) can sit below the model, which holds the camera in `AppState`, without the model or the rules passing the size at which
//! a crate has to be split into modules.

mod input;

pub use input::{Modifiers, Point};
