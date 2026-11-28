//! The tab of the right card and the question of the Activity panel as state of the app (audit finding T10, issue 323):
//! a question is a pending question, source-agnostic UI state (rule U7; not an **Approval** of the glossary, which is the
//! owner's answer to an Ask call): nothing is reverted or restored until it is confirmed, it is answered once, and one that
//! is dropped unanswered (its row gone, the selection moved to another batch, its checkpoint gone, the tab changed) is closed,
//! while the revert and the restore themselves are `review.rs` and the fetch is `activity.rs`.

use crate::app::App;
use crate::document::refuse;
use crate::effect::Effect;
use crate::event::Event;
use meldframe_app_model::{Action, ActivityOp, BatchRef, Confirm, RightTab};
use meldframe_sync::SubmitError;

impl App {
    /// An op that changes the view only: the tab, a question about a checkpoint, closing a question.
    pub(crate) fn activity_view(&mut self, op: ActivityOp, fx: &mut Vec<Effect>) {
        match op {
            ActivityOp::SetTab { tab } => {
                if self.state.right_tab == tab {
                    return;
                }
                self.state.right_tab = tab;
                // The question is drawn in the Activity tab: leaving the tab takes the screen away, and a question whose screen
                // is gone is denied (rule U7), never left to be answered by a click on a tab that comes back.
                self.state.confirm = None;
                self.events.push(Event::RightTabChanged { tab });
                if tab == RightTab::Activity {
                    self.activity_action(Action::OpenActivity {}, fx);
                }
            }
            ActivityOp::AskRestore { id } => {
                if self.state.checkpoints.iter().any(|c| c.id == id) {
                    self.state.confirm = Some(Confirm::Restore { id });
                } else {
                    drop(refuse(&mut self.events, "unknown_checkpoint"));
                }
            }
            ActivityOp::Cancel {} => self.state.confirm = None,
            // The three that select, revert or restore are the document's (`document`).
            ActivityOp::AskRevert { .. }
            | ActivityOp::ConfirmRevert { .. }
            | ActivityOp::ConfirmRestore { .. } => {}
        }
    }

    /// An op that selects a batch, reverts one or restores a checkpoint.
    pub(crate) fn activity_change(
        &mut self,
        op: ActivityOp,
        fx: &mut Vec<Effect>,
    ) -> Result<(), SubmitError> {
        match op {
            ActivityOp::AskRevert { batch } => {
                match self.feed.select(&batch.actor, &batch.key_prefix) {
                    Some(ids) => {
                        self.select(ids);
                        self.state.confirm = Some(Confirm::revert(&batch));
                        Ok(())
                    }
                    None => refuse(&mut self.events, "unknown_batch"),
                }
            }
            // Only for the batch the question is about, and only while it is the selected one: the answer on the screen was
            // worked out for the selected row.
            ActivityOp::ConfirmRevert { batch } => {
                let asked = self
                    .state
                    .confirm
                    .as_ref()
                    .is_some_and(|c| c.is_revert_of(&batch));
                if !asked || self.state.activity.selected.as_ref() != Some(&batch) {
                    return Ok(());
                }
                self.state.confirm = None;
                self.revert_batch(&batch.actor, &batch.key_prefix, fx)
            }
            ActivityOp::ConfirmRestore { id } => {
                if self.state.confirm != Some(Confirm::Restore { id: id.clone() }) {
                    return Ok(());
                }
                self.state.confirm = None;
                self.restore_checkpoint(&id, fx)
            }
            _ => Ok(()),
        }
    }

    /// Showing another batch ends a question to revert one: the selection moves away from it.
    pub(crate) fn confirm_follows_selection(&mut self, shown: &BatchRef) {
        if self
            .state
            .confirm
            .as_ref()
            .is_some_and(|c| matches!(c, Confirm::Revert { .. }) && !c.is_revert_of(shown))
        {
            self.state.confirm = None;
        }
    }

    /// A question belongs to a row that is still there and still the selected one, and to a checkpoint that still exists.
    pub(crate) fn settle_confirm(&mut self) {
        let holds = match &self.state.confirm {
            None => return,
            Some(Confirm::Restore { id }) => self.state.checkpoints.iter().any(|c| c.id == *id),
            Some(c @ Confirm::Revert { .. }) => {
                let a = &self.state.activity;
                a.selected.as_ref().is_some_and(|s| c.is_revert_of(s))
                    && a.rows.iter().any(|r| {
                        c.is_revert_of(&BatchRef {
                            actor: r.actor.clone(),
                            key_prefix: r.key_prefix.clone(),
                        })
                    })
            }
        };
        if !holds {
            self.state.confirm = None;
        }
    }
}
