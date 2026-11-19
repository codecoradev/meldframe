//! Limits and validation of node values and commands. Validation is separate from application so that
//! the checks can be read, tested and reused on their own (the server's gate, the agent tools).

mod error;
mod limits;
mod lint;
mod load;
mod structure;
mod validate;

pub use error::ApplyError;
pub use limits::Limits;
pub use lint::{
    MAX_NAME_WARNINGS, NameFinding, default_name_findings, is_default_layer_name,
    name_warnings_of_commands, name_warnings_of_nodes,
};
pub use load::{LoadError, check_file, check_file_with};
pub use structure::{check_command, check_command_with, position};
pub use validate::{check_node, check_patch, check_pos};
