//! Document actions: what the person meant becomes commands in the replica.

use crate::action::{Action, CreateKind, Property};
use crate::app::App;
use crate::effect::Effect;
use crate::state::Tool;
use meldframe_app_parts::document::{code, new_node, update};
use meldframe_app_parts::touched::render_set;
use meldframe_check::{ApplyError, Limits, check_command_with};
use meldframe_protocol::{Command, NodeKind, NodePatch};
use meldframe_sync::{Replica, SubmitError};
use meldframe_variant::VariantError;

/// Refuse an action with a stable code; nothing changed.
pub(crate) fn refuse(events: &mut Vec<crate::Event>, code: &str) -> Result<(), SubmitError> {
    events.push(crate::Event::Refused { code: code.into() });
    Ok(())
}

pub(crate) fn apply_all(r: &mut Replica, cmds: &[Command]) -> Result<(), SubmitError> {
    r.begin_group();
    let done = cmds.iter().try_for_each(|c| r.apply(c));
    r.end_group();
    done
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
                let (w, h) = preset.size();
                let mut node = new_node(&id, CreateKind::Frame, x, y, parent, Some(w), Some(h));
                node.name = preset.name().into();
                let done = self.replica.apply(&Command::AddNode { node, index: None });
                select = done.is_ok().then_some(id);
                done
            }
            Action::MakeVariant { id } => {
                let (prefix, mut n) = (self.prefix.clone(), self.next_id);
                let made = meldframe_variant::variant_of(
                    self.replica.nodes(),
                    &id,
                    || {
                        n += 1;
                        format!("{prefix}-n{n}")
                    },
                    &Limits::default(),
                );
                self.next_id = n;
                // The whole copy was applied to a copy of the file already, so it applies here as one step.
                match made {
                    Ok(v) => self.check_room(&v.commands).and_then(|()| {
                        let done = apply_all(&mut self.replica, &v.commands);
                        select = done.is_ok().then_some(v.root);
                        done
                    }),
                    Err(VariantError::Apply(e)) => Err(e.into()),
                    Err(VariantError::NotFound) => Err(ApplyError::NotFound(id).into()),
                    Err(VariantError::NotAFrame) => {
                        self.events.push(crate::Event::Refused {
                            code: "not_a_frame".into(),
                        });
                        Ok(())
                    }
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
            Action::Move { id, x, y } => self.replica.apply(&update(
                &id,
                NodePatch {
                    x: Some(x),
                    y: Some(y),
                    ..Default::default()
                },
            )),
            Action::MoveBy { dx, dy } => {
                let cmds: Vec<Command> = self
                    .state
                    .selection
                    .iter()
                    .filter_map(|id| self.replica.nodes().iter().find(|n| n.id == *id))
                    .map(|n| {
                        update(
                            &n.id,
                            NodePatch {
                                x: Some(n.x + dx),
                                y: Some(n.y + dy),
                                ..Default::default()
                            },
                        )
                    })
                    .collect();
                match self.validate(&cmds) {
                    Ok(()) => apply_all(&mut self.replica, &cmds),
                    Err(e) => Err(e),
                }
            }
            Action::Resize { id, w, h, x, y } => self.replica.apply(&update(
                &id,
                NodePatch {
                    x,
                    y,
                    w: Some(w),
                    h: Some(h),
                    ..Default::default()
                },
            )),
            Action::EditText { id, text } => self.replica.apply(&update(
                &id,
                NodePatch {
                    text: Some(text),
                    ..Default::default()
                },
            )),
            Action::SetProperty { id, property } => {
                // Only a frame clips: the canvas draws nothing else that way, so other kinds refuse the field.
                let other_kind = self
                    .nodes()
                    .iter()
                    .any(|n| n.id == id && n.kind != NodeKind::Frame);
                if matches!(property, Property::Clip(_)) && other_kind {
                    refuse(&mut self.events, "invalid")
                } else {
                    let mut patch = NodePatch::default();
                    match property {
                        Property::Name(v) => patch.name = Some(v),
                        Property::Fill(v) => patch.fill = Some(v.into()),
                        Property::Radius(v) => patch.radius = Some(v.into()),
                        Property::FontSize(v) => patch.font_size = Some(v),
                        Property::Opacity(v) => patch.opacity = Some(v.into()),
                        Property::Clip(v) => patch.clip = Some(v),
                    }
                    self.replica.apply(&update(&id, patch))
                }
            }
            Action::Delete { ids } => self.delete(&ids),
            Action::Reorder { id, before, after } => self.replica.apply(&Command::Reorder {
                id,
                before,
                after,
                pos: None,
            }),
            Action::RevertBatch { actor, key_prefix } => self.revert_batch(&actor, &key_prefix),
            Action::CreateCheckpoint { label } => self.create_checkpoint(label),
            Action::RestoreCheckpoint { id } => self.restore_checkpoint(&id),
            Action::EditVariables { commands } => self.edit_variables(&commands),
            Action::SetVariableValue {
                variable,
                mode,
                value,
            } => self.set_variable_value(variable, mode, value),
            Action::BindProperty {
                id,
                field,
                variable,
            } => self.bind_property(&id, field, variable),
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
    fn validate(&self, cmds: &[Command]) -> Result<(), SubmitError> {
        self.check_room(cmds)?;
        let limits = Limits::default();
        cmds.iter()
            .try_for_each(|c| {
                check_command_with(self.replica.nodes(), self.replica.vars(), c, &limits)
            })
            .map_err(SubmitError::Invalid)
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
