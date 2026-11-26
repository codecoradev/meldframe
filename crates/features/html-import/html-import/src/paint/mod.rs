//! Paint (E4.4, `docs/import-html.md` section 3.6): the fields of a Node that are only seen, read from the computed style of one element.
//!
//! [`read`] is a pure function of that style: no tree, no mapping, no other stage. The structure stage calls it once
//! for each Node it makes and [`Paint::apply`] puts the answer on the Node.
//!
//! What is read, and what the model holds (`meldframe-protocol`):
//!
//! - `background-color` and the image layers of `background-image`: `fill`, one linear or radial `gradient` and one
//!   `image`. The image is read only in the form the export writes (an asset id in the url, with the size, position
//!   and repeat the export writes), every other `url()` is a placeholder: nothing is fetched, the text is kept in
//!   [`Paint::assets`];
//! - `border` (four equal solid sides, which take room: [`Paint::border_room`]) and the border of the export, the
//!   `::after` box, with its position;
//! - `border-radius` (four equal corners; `50%` is an ellipse or half the smaller side);
//! - `box-shadow` (outer shadows, the four topmost);
//! - `opacity`.
//!
//! Whatever else of these properties is declared is refused, never guessed, and goes to the report with the byte of
//! the element: kind `Refused` for a property that is lost, `Listed` for a value that was changed to the nearest the
//! model holds, `Cut` when a bound stopped the reading. The properties of the style stage (`Style::notes`: a
//! shorthand that was not read, a `var()` that did not resolve) are not repeated here. [`handles`] says which
//! property names this stage has an answer for, so that another stage does not list them again.

mod border;
mod calc;
mod fill;
mod gradient;
mod image;
mod lex;
mod misc;
mod model;
mod radius;
mod reader;
mod shadow;

pub use misc::handles;
pub use model::{Asset, Ctx, Paint};
pub(crate) use reader::Sink;
pub use reader::read;
