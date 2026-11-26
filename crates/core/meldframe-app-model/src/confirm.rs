//! The question a person is being asked in the Activity panel, and the tab of the right card (audit finding T10, issue
//! 323). Both are view state with rules, so they are state of the app layer: a second front end (a headless one among
//! them) can open the Activity tab, ask to revert a batch and confirm it without any of the rules moving.

use crate::activity::BatchRef;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The two tabs of the right card.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum RightTab {
    /// The properties of the selection and the Variables panel.
    #[default]
    Design,
    /// The lanes of batches and the checkpoints. Changing to it asks for the newest batches (`ActivityOp::SetTab` of `Action::Activity`).
    Activity,
}

/// The one question the person is being asked: revert a batch, or restore a checkpoint. It is the pending question of the
/// Activity panel, source-agnostic UI state as rule U7 says of a pending request: nothing is reverted or restored until it
/// is confirmed, and a question whose screen is gone (its row is gone, the selection moved to another batch, its
/// checkpoint is gone, the tab changed) is closed, never answered. It is not an **Approval** of the glossary (the owner's
/// answer to a call whose policy is Ask); it is the person's own confirmation of their own click.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Confirm {
    /// Revert this batch.
    Revert { actor: String, key_prefix: String },
    /// Restore the checkpoint with this id.
    Restore { id: String },
}

impl Confirm {
    /// The question that asks to revert `batch`.
    pub fn revert(batch: &BatchRef) -> Self {
        Confirm::Revert {
            actor: batch.actor.clone(),
            key_prefix: batch.key_prefix.clone(),
        }
    }

    /// Whether this is the question about `batch`.
    pub fn is_revert_of(&self, batch: &BatchRef) -> bool {
        matches!(self, Confirm::Revert { actor, key_prefix } if *actor == batch.actor && *key_prefix == batch.key_prefix)
    }
}

/// What a person does with the tab of the right card and with the question of the Activity panel (`Action::Activity`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ActivityOp {
    /// Select a tab. Only a change to Activity asks for the newest batches and for the checkpoints (heal rule (c), a person's
    /// action, as `Action::OpenActivity`); the tab that is selected already asks for nothing, so a second click or a key on it
    /// does not fetch again. The fetch goes out only while the app is live; the tab changes either way.
    SetTab { tab: RightTab },
    /// Ask whether to revert a batch: select it (as `Action::SelectBatch` does, so the app works out whether a revert would
    /// still change anything) and open the question. Refused as `unknown_batch` for a batch the app does not hold.
    AskRevert { batch: BatchRef },
    /// Go ahead with the revert (`Action::RevertBatch`), once, and close the question. Nothing unless the question is about
    /// this batch and it is still the selected one: the answer on the screen was worked out for the selected row.
    ConfirmRevert { batch: BatchRef },
    /// Ask whether to restore a checkpoint. Refused as `unknown_checkpoint` for one the app does not hold.
    AskRestore { id: String },
    /// Go ahead with the restore (`Action::RestoreCheckpoint`), once, and close the question. Nothing unless the question
    /// is about this checkpoint.
    ConfirmRestore { id: String },
    /// Close the question (Cancel, Close and Esc).
    Cancel {},
}

impl ActivityOp {
    /// Whether the op changes the view only: no op is sent, no batch is selected, the file does not change.
    pub fn is_view(&self) -> bool {
        matches!(
            self,
            ActivityOp::SetTab { .. } | ActivityOp::AskRestore { .. } | ActivityOp::Cancel {}
        )
    }
}
