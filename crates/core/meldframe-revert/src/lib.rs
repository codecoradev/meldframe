//! Revert of a batch (E5.2) and checkpoints (E5.3): pure functions over a tree and a slice of the log.

mod checkpoint;
mod record;
mod restore;
mod revert;

pub use checkpoint::{RUN_GAP_MS, RunDetector, starts_run};
pub use record::{ConfirmedLog, FOLD_CHUNK, MAX_LOG};
pub use restore::{RestoreError, restore_commands, state_at};
pub use revert::{
    Batch, LoggedOp, Revert, RevertError, SkipReason, Skipped, batch_index, revert_batch,
};
