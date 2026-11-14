//! `Replica`: the client side of live editing. It keeps a confirmed copy of the file, applies my
//! commands at once as pending ops, and follows the server's total order of ops.

mod replica;
mod types;

pub use replica::Replica;
pub use types::{EventOutcome, OpKind, Outgoing, ReplicaError};
