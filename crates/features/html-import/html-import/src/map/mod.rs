//! The mapping (E4.4, `docs/import-html.md` sections 3.1 to 3.5 and 4.3): the styled tree to a list of `addNode` commands, with the report of what each node could not take.
//!
//! [`import`] is the whole importer as a pure function of the text: it parses, computes the styles, maps, and bounds
//! the result at [`MAX_COMMANDS`].
//!
//! - **Structure** (`walk.rs`). An element is a box (a node) or part of a run of text. A `display: none` element, and
//!   the ones a browser never draws, make nothing; `display: contents` and inline elements pass their content to the
//!   element they are in. The text of a run is one `text` node with the style of the box that holds it (the model has
//!   one style per text: a different inline style is listed on the node). An element whose content is one run and
//!   that paints nothing of its own is that text node; with a background, border, shadow or padding it is a frame with
//!   a text in it. A wrapper that only holds one box and says nothing of its own makes no node.
//! - **Layout and sizing** (`size.rs`, `node.rs`). A flex container is a row or a column frame, a block is a column
//!   with no gap, and a frame whose children are all absolute has no layout. A size is fixed, hug or fill by the
//!   rules of section 3.4 (rule G1: no fill under a parent that hugs on that axis). The fields the model defaults
//!   differently from CSS are always written (section 3.1).
//! - **Text** (`text.rs`). Spaces collapse as the browser collapses them, `br` is a line break, and the text of a node
//!   is file content: it is kept as written, never cleaned. Names the importer makes up, and everything the report
//!   echoes, go through `meldframe-text` (rule U9).
//! - **Paint** is the paint stage (`crate::paint`), read once for each box.
//!
//! The result is bounded: more than [`MAX_COMMANDS`] nodes is [`ImportError::TooMany`] and nothing is made. Nothing is
//! fetched or run; the stage is a function of the text and builds for `wasm32-unknown-unknown`.

mod items;
mod leaf;
mod model;
mod names;
mod node;
mod notes;
mod page;
mod refuse;
mod run;
mod size;
mod text;
mod walk;

pub use model::{
    AssetRef, ImportError, Imported, MAX_COMMANDS, MAX_INPUT_BYTES, MAX_SKIPPED, NodeInfo, Options,
    Skipped,
};
pub use notes::{Change, Lost};
pub use run::import;
