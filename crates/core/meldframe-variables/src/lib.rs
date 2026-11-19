//! Variables of a file: resolving a binding through aliases and modes (one function for the canvas, layout
//! and export, ADR-0004), the alias graph, who uses a variable, and applying and inverting the variable
//! commands. Pure data in, data out; the checks that refuse a command are in `meldframe-check`.

mod alias;
mod bindings;
mod error;
mod limits;
mod resolve;

pub use alias::{Aliases, alias_depth, alias_height, alias_reaches};
pub use bindings::{Binding, node_bindings, patch_bindings, users};
pub use error::{VAR_ERROR_CODES, VarError};
pub use limits::VarLimits;
pub use resolve::{MAX_ALIAS_DEPTH, ModeChoice, Resolver, resolve};
