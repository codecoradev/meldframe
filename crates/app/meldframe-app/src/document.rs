//! Document actions: what the person meant becomes commands in the replica.

use crate::action::{Action, CreateKind, Property};
use crate::app::{App, render_set};
use crate::effect::Effect;
use crate::state::Tool;
use meldframe_check::{ApplyError, Limits, check_command};
use meldframe_protocol::{Command, Node, NodePatch};
use meldframe_sync::{Replica, SubmitError};
use serde_json::json;

fn update(id: &str, patch: NodePatch) -> Command {
    Command::UpdateNode {
        id: id.to_string(),
        patch,
    }
}

/// A stable code for a refusal; the front end words the message.
fn code(e: &SubmitError) -> &'static str {
    let SubmitError::Invalid(e) = e else {
        return "outbox_full";
    };
    match e {
        ApplyError::NotFound(_) => "not_found",
        ApplyError::DuplicateId(_) => "duplicate_id",
        ApplyError::ParentNotFound(_) | ApplyError::ParentNotFrame(_) => "bad_parent",
        ApplyError::Cycle => "cycle",
        ApplyError::TooManyNodes(_)
        | ApplyError::TooDeep(_)
        | ApplyError::TooLong { .. }
        | ApplyError::TooMany { .. }
        | ApplyError::PosTooLong { .. } => "limit",
        _ => "invalid",
    }
}

fn apply_all(r: &mut Replica, cmds: &[Command]) -> Result<(), SubmitError> {
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
            Action::CreateNode { kind, x, y, parent } if !(x.is_finite() && y.is_finite()) => {
                let _ = (kind, parent);
                Err(ApplyError::NotFinite("x").into())
            }
            Action::CreateNode { kind, x, y, parent } => {
                let id = self.fresh_id();
                let cmd = Command::AddNode {
                    node: new_node(&id, kind, x, y, parent),
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
            Action::Resize { id, w, h } => self.replica.apply(&update(
                &id,
                NodePatch {
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
                let mut patch = NodePatch::default();
                match property {
                    Property::Name(v) => patch.name = Some(v),
                    Property::Fill(v) => patch.fill = Some(v),
                    Property::Radius(v) => patch.radius = Some(v),
                    Property::FontSize(v) => patch.font_size = Some(v),
                }
                self.replica.apply(&update(&id, patch))
            }
            Action::Delete { ids } => self.delete(&ids),
            Action::Reorder { id, before, after } => self.replica.apply(&Command::Reorder {
                id,
                before,
                after,
                pos: None,
            }),
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

    /// Every command is valid on the current document and the queue has room for all of them: checked
    /// before any is applied, so a list that would fail half way applies nothing.
    fn validate(&self, cmds: &[Command]) -> Result<(), SubmitError> {
        let max = self.replica.max_pending();
        if self.replica.pending_len() + cmds.len() > max {
            return Err(SubmitError::OutboxFull { max });
        }
        let limits = Limits::default();
        cmds.iter()
            .try_for_each(|c| check_command(self.replica.nodes(), c, &limits))
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

fn new_node(id: &str, kind: CreateKind, x: f64, y: f64, parent: Option<String>) -> Node {
    let kind = match kind {
        CreateKind::Frame => "frame",
        CreateKind::Rectangle => "rect",
        CreateKind::Text => "text",
    };
    serde_json::from_value(json!({"id": id, "type": kind, "parentId": parent, "x": x, "y": y}))
        .expect("a node with these fields is valid")
}
