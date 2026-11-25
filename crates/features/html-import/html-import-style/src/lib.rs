//! HTML import, style stage (E4.4, ADR-0024, `docs/import-html.md` section 3.3): what is done with the declarations
//! the CSS stage (`html-import-css`) reads and the cascade orders. It is a crate of its own so that none of the
//! importer's crates passes the size ceiling. Pure functions of the text, bounded, with the work counted.
//!
//! - [`value`]: a length in pixels, a colour as hex, a value cut into words;
//! - [`vars`]: `var()` replaced by the literal, with what it cost;
//! - [`shorthand`]: shorthands expanded to the longhands the mapping reads;
//! - [`defaults`]: the browser's defaults table and the inherited properties;
//! - [`style_tree`], [`compute`] and [`Style`]: the computed style of every element of the tree (`style.rs`).

mod computed;
mod custom;
pub mod defaults;
pub mod shorthand;
mod style;
pub mod value;
pub mod vars;

pub use computed::{Custom, MAX_NOTES, Note, Prop, Props, Source, Style};
pub use style::{Styles, compute, style_tree};
