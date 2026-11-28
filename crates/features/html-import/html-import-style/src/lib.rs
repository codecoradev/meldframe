//! HTML import, style stage (E4.4, ADR-0024, `docs/import-html.md` section 3.3): what is done with the declarations the CSS side reads and the cascade orders, as pure functions of the text that are bounded and count their work.
//!
//! - [`value`]: a length in pixels, a colour as hex, a value cut into words;
//! - [`vars`]: `var()` replaced by the literal, with what it cost;
//! - [`shorthand`]: shorthands expanded to the longhands the mapping reads;
//! - [`defaults`]: the browser's defaults table and the inherited properties;
//! - `styles.rs`: [`style_tree`], [`compute`] and [`Style`]: the computed style of every element of the tree.

mod computed;
mod custom;
pub mod defaults;
pub mod shorthand;
mod styles;
pub mod value;
pub mod vars;

pub use computed::{Custom, MAX_NOTES, Note, Prop, Props, Source, Style};
pub use styles::{Styles, compute, style_tree};
