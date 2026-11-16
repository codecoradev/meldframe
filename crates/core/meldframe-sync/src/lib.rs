//! `Replica`: the client side of live editing. It keeps a confirmed copy of the file, applies my
//! commands at once as pending ops, and follows the server's total order of ops.

mod correction;
mod footprint;
mod replica;
mod types;

pub use meldframe_check::LoadError;
pub use replica::{DEFAULT_MAX_PENDING, Replica};
pub use types::{EventOutcome, OpKind, Outgoing, ReplicaError, ResyncError, SubmitError};
