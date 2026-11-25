//! HTML import, style stage (E4.4, ADR-0024, `docs/import-html.md` section 3.3): what is done with the declarations
//! the CSS stage (`html-import-css`) reads and the cascade orders. It is a crate of its own so that none of the
//! importer's crates passes the size ceiling. Pure functions of the text, bounded, with the work counted.
//!
//! - [`value`]: a length in pixels, a colour as hex, a value cut into words;
//! - [`vars`]: `var()` replaced by the literal, with what it cost;
//! - [`shorthand`]: shorthands expanded to the longhands the mapping reads;
//! - [`defaults`]: the browser's defaults table and the inherited properties.

pub mod defaults;
pub mod shorthand;
pub mod value;
pub mod vars;
