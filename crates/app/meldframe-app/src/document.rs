//! Document actions: what the person meant becomes commands in the replica.

use crate::action::Action;
use crate::app::App;
use crate::effect::Effect;
use crate::state::Tool;
use meldframe_app_model::{ManageOp, TypedText};
use meldframe_app_parts::document::{
    Variant, apply_all, check_all, code, edit_text, make_variant, move_by, move_to, new_node,
    preset_node,
};
use meldframe_app_parts::manage::Target;
use meldframe_app_parts::touched::render_set;
use meldframe_check::ApplyError;
use meldframe_protocol::Command;
use meldframe_sync::SubmitError;

/// Refuse an action with a stable code; nothing changed.
pub(crate) fn refuse(events: &mut Vec<crate::Event>, code: &str) -> Result<(), SubmitError> {
    events.push(crate::Event::Refused { code: code.into() });
    Ok(())
}

impl App {
    pub(crate) fn document(&mut self, action: Action, fx: &mut Vec<Effect>) {
        let before = self.replica.pending_len();
        let mut select = None;
        let result = match action {
            Action::CreatePreset { x, y, .. } if !(x.is_finite() && y.is_finite()) => {
                Err(ApplyError::NotFinite("x or y").into())
            }
            Action::CreatePreset {
                preset,
                x,
                y,
                parent,
            } => {
                let id = self.fresh_id();
                let node = preset_node(&id, preset, x, y, parent);
                let done = self.replica.apply(&Command::AddNode { node, index: None });
                select = done.is_ok().then_some(id);
                done
            }
            Action::MakeVariant { id } => {
                match make_variant(
                    self.replica.nodes(),
                    self.replica.vars(),
                    &id,
                    &self.prefix,
                    &mut self.next_id,
                ) {
                    // The whole copy was applied to a copy of the file already, so it applies here as one step.
                    Ok(Variant::Made { commands, root }) => {
                        self.check_room(&commands).and_then(|()| {
                            let done = apply_all(&mut self.replica, &commands);
                            select = done.is_ok().then_some(root);
                            done
                        })
                    }
                    Ok(Variant::NotAFrame) => {
                        self.events.push(crate::Event::Refused {
                            code: "not_a_frame".into(),
                        });
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            Action::CreateNode { x, y, w, h, .. }
                if ![Some(x), Some(y), w, h]
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite()) =>
            {
                Err(ApplyError::NotFinite("x, y, w or h").into())
            }
            Action::CreateNode {
                kind,
                x,
                y,
                parent,
                w,
                h,
            } => {
                let id = self.fresh_id();
                let cmd = Command::AddNode {
                    node: new_node(&id, kind, x, y, parent, w, h),
                    index: None,
                };
                let done = self.replica.apply(&cmd);
                select = done.is_ok().then_some(id);
                done
            }
            Action::Move { id, x, y } => self.replica.apply(&move_to(&id, x, y)),
            Action::MoveBy { dx, dy } => {
                let cmds = move_by(&self.state.selection, self.replica.nodes(), dx, dy);
                match self.validate(&cmds) {
                    Ok(()) => apply_all(&mut self.replica, &cmds),
                    Err(e) => Err(e),
                }
            }
            Action::Resize { id, w, h, x, y } => self.resize_node(&id, (w, h), (x, y)),
            Action::DropNode { id, x, y, dx, dy } => self.drop_node(&id, (x, y), (dx, dy)),
            Action::DropNodes { ids, x, y, dx, dy } => self.drop_nodes(&ids, (x, y), (dx, dy)),
            Action::ArrowKey { direction } => self.arrow_key(direction),
            Action::EditText { id, text } => self.replica.apply(&edit_text(&id, text)),
            Action::SetProperty { id, property } => self.set_property(&id, property),
            Action::Typed { entry } => match entry {
                TypedText::Property { id, field, text } => {
                    self.set_property_text(&id, &field, &text)
                }
                TypedText::VariableValue {
                    variable,
                    mode,
                    text,
                } => self.set_variable_value_text(variable, mode, text),
            },
            Action::Manage { op } => match op {
                ManageOp::ConfirmRemove {} => self.confirm_remove(),
                ManageOp::SubmitRename {} => self.submit_rename(),
                _ => Ok(()),
            },
            Action::SetImageFill { id, asset_id, fit } => self.set_image_fill(&id, asset_id, fit),
            Action::Delete { ids } => self.delete(&ids),
            Action::Reorder { id, before, after } => self.replica.apply(&Command::Reorder {
                id,
                before,
                after,
                pos: None,
            }),
            Action::RevertBatch { actor, key_prefix } => self.revert_batch(&actor, &key_prefix, fx),
            Action::CreateCheckpoint { label } => self.create_checkpoint(&label, fx),
            Action::RestoreCheckpoint { id } => self.restore_checkpoint(&id, fx),
            Action::CheckpointPosted { key, checkpoint } => {
                self.checkpoint_posted(&key, checkpoint)
            }
            Action::EditVariables { commands } => self.edit_variables(&commands),
            Action::ImportVariables { text } => self.import_variables(&text),
            Action::CreateCollection { name } => self.create_collection(&name),
            Action::AddMode {
                collection,
                name,
                copy_from,
            } => self.add_mode(&collection, &name, copy_from),
            Action::RemoveMode { collection, mode } => self.remove_mode(collection, mode),
            Action::AddVariable {
                collection,
                name,
                kind,
            } => self.add_variable(&collection, &name, kind),
            Action::RemoveVariable { id, detach } => {
                self.remove_entity(Target::Variable(&id), detach)
            }
            Action::RemoveCollection { id, detach } => {
                self.remove_entity(Target::Collection(&id), detach)
            }
            Action::SetVariableValue {
                variable,
                mode,
                value,
            } => self.set_variable_value(variable, mode, value),
            Action::BindProperty {
                id,
                field,
                variable,
            } => {
                let done = self.bind_property(&id, field, variable);
                // The field is settled: the picker that was open on it has done its work. A bind that was refused
                // leaves it open, so the person can pick another.
                if done.is_ok()
                    && self
                        .state
                        .picker
                        .as_ref()
                        .is_some_and(|p| p.id == id && p.field == field)
                {
                    self.close_picker();
                }
                done
            }
            Action::Undo {} => self.step(true),
            Action::Redo {} => self.step(false),
            // The other actions are not about the document.
            _ => Ok(()),
        };
        if let Err(e) = result {
            self.events.push(crate::Event::Refused {
                code: code(&e).to_string(),
            });
        }
        let made = self.replica.unconfirmed().split_off(before);
        if made.is_empty() {
            return;
        }
        let commands: Vec<Command> = made.into_iter().map(|o| o.command).collect();
        let nodes = render_set(&commands);
        let me = self.replica.actor().to_string();
        self.changed(me, true, nodes, fx);
        if let Some(id) = select {
            self.select(vec![id]);
            self.set_tool(Tool::Select);
        }
    }

    /// Whether a list action of `cmds` fits the queue of pending ops, before anything is applied.
    pub(crate) fn check_room(&self, cmds: &[Command]) -> Result<(), SubmitError> {
        let max = self.replica.max_pending();
        if self.replica.pending_len() + cmds.len() > max {
            return Err(SubmitError::OutboxFull { max });
        }
        Ok(())
    }

    /// Every command is valid on the current document and the queue has room for all of them: checked
    /// before any is applied, so a list that would fail half way applies nothing.
    pub(crate) fn validate(&self, cmds: &[Command]) -> Result<(), SubmitError> {
        self.check_room(cmds)?;
        check_all(self.replica.nodes(), self.replica.vars(), cmds)
    }

    fn delete(&mut self, ids: &[String]) -> Result<(), SubmitError> {
        let cmds: Vec<Command> = ids
            .iter()
            .map(|id| Command::RemoveNode { id: id.clone() })
            .collect();
        self.validate(&cmds)?;
        self.replica.begin_group();
        let mut done = Ok(());
        for id in ids {
            // A node under an earlier id of the list is gone already.
            if self.replica.nodes().iter().any(|n| n.id == *id) {
                done = self.replica.apply(&Command::RemoveNode { id: id.clone() });
                if done.is_err() {
                    break;
                }
            }
        }
        self.replica.end_group();
        done
    }

    /// Undo or redo one step; nothing when the history has none.
    fn step(&mut self, undo: bool) -> Result<(), SubmitError> {
        if undo {
            self.replica.undo().map(drop)
        } else {
            self.replica.redo().map(drop)
        }
    }
}
