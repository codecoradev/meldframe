//! Revert of a batch (E5.2) and checkpoints (E5.3): pure functions over a tree and a slice of the log.

mod revert;

pub use revert::{Batch, LoggedOp, Revert, RevertError, SkipReason, Skipped, revert_batch};
