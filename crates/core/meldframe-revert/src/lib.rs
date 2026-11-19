//! Revert of a batch (E5.2) and checkpoints (E5.3): pure functions over a tree and a slice of the log.

mod checkpoint;
mod restore;
mod revert;

pub use checkpoint::RunDetector;
pub use restore::{RestoreError, restore_commands, state_at};
pub use revert::{Batch, LoggedOp, Revert, RevertError, SkipReason, Skipped, revert_batch};
