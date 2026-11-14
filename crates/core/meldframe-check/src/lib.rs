//! Limits and validation of node values and commands. Validation is separate from application so that
//! the checks can be read, tested and reused on their own (the server's gate, the agent tools).

mod error;
mod limits;
mod validate;

pub use error::ApplyError;
pub use limits::Limits;
pub use validate::{check_node, check_patch, check_pos};
