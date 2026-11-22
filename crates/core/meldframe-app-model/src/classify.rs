//! The classification of an action: which rules handle it, and not the document (`meldframe-app`: `activity.rs` and
//! `checkpoints.rs`). Plain matches, no rule.

use crate::action::Action;

impl Action {
    /// Whether the activity rules handle it (`meldframe-app`, `activity.rs`) and not the document.
    pub fn is_activity(&self) -> bool {
        matches!(
            self,
            Action::OpenActivity {}
                | Action::LoadOlderBatches {}
                | Action::SelectBatch { .. }
                | Action::SetBatchReview { .. }
                | Action::BatchesLoaded { .. }
                | Action::BatchReviewed { .. }
                | Action::BatchRequestFailed { .. }
        )
    }

    /// Whether the checkpoint rules handle it (`meldframe-app`, `checkpoints.rs`): the answers that change no node.
    pub fn is_checkpoint(&self) -> bool {
        matches!(
            self,
            Action::CheckpointsLoaded { .. } | Action::CheckpointRequestFailed { .. }
        )
    }

    /// An activity action that does not touch the document, the selection or the properties panel (all but
    /// `SelectBatch`): the app has only the panel to bring up to date.
    pub fn only_activity(&self) -> bool {
        self.is_activity() && !matches!(self, Action::SelectBatch { .. })
    }
}
