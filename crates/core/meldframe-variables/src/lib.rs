//! Variables of a file: resolving a binding through aliases and modes (one function for the canvas, layout
//! and export, ADR-0004), the alias graph, who uses a variable, and applying and inverting the variable
//! commands. Pure data in, data out; the checks that refuse a command are in `meldframe-check`.

mod alias;
mod apply;
mod bindings;
mod check;
mod error;
mod footprint;
mod limits;
mod resolve;

pub use alias::{Aliases, alias_depth, alias_height, alias_reaches};
pub use apply::{apply, invert, is_variable_command, touched};
pub use bindings::{Binding, node_bindings, patch_bindings, users};
pub use check::{
    check_file_bindings, check_node_bindings, check_patch_bindings, check_variable_command,
    check_variables, is_name_path,
};
pub use error::{VAR_ERROR_CODES, VarError};
pub use footprint::{Pairs, footprint};
pub use limits::VarLimits;
pub use resolve::{MAX_ALIAS_DEPTH, ModeChoice, Resolver, resolve};
