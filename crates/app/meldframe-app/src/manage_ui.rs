//! The Variables manage view as state of the app (audit finding T5, issue 322): which collection is shown, the question
//! before a removal, the field a name is edited in, and the text of the forms that add something. The rules that read the
//! file are `meldframe_app_parts::manage_names`; the commands of a removal are `meldframe_app_parts::manage`, run by the
//! actions of `manage.rs`. This holds the transitions, and `refresh_manage` brings what follows the file (the checks, the
//! numbers of a question, a field whose target another actor removed) up to date once per dispatch.

use crate::app::App;
use crate::document::refuse;
use crate::event::Event;
use meldframe_app_model::{
    ManageDrafts, ManageOp, ManageTarget, NameDraft, NameError, RenameField, VariableEntity,
};
use meldframe_app_parts::manage::Target;
use meldframe_app_parts::manage_names::{
    check_name, check_rename, gone_code, removal_of, rename_context,
};
use meldframe_protocol::{Command, Variables};
use meldframe_sync::SubmitError;
use meldframe_text::{MAX_LABEL, clip, label};

/// The collection the view shows: the one chosen, else the first.
fn shown<'a>(vars: &'a Variables, chosen: Option<&String>) -> Option<&'a str> {
    chosen
        .and_then(|id| vars.collection(id))
        .or_else(|| vars.collections.first())
        .map(|c| c.id.as_str())
}

/// How many times the longest name a pasted text may be before it is cut: the text is kept so the person can see and cut
/// it, and its check says `TooLong`, but a text of any size is not held in the state.
const KEEP_NAMES: usize = 4;

/// A typed text as the state keeps it: whole up to `KEEP_NAMES` times `name_max` characters, cut there beyond.
fn kept(text: String, name_max: usize) -> String {
    match clip(&text, KEEP_NAMES * name_max, usize::MAX) {
        c if c.len() == text.len() => text,
        c => c.to_string(),
    }
}

/// The names a new thing of this kind is compared with: the collections of the file, the modes or the variables of the
/// collection that is shown.
fn existing(vars: &Variables, shown: Option<&str>, form: VariableEntity) -> Vec<String> {
    let in_shown = shown.and_then(|id| vars.collection(id));
    match form {
        VariableEntity::Collection => vars.collections.iter().map(|c| c.name.clone()).collect(),
        VariableEntity::Mode => in_shown
            .map(|c| c.modes.iter().map(|m| m.name.clone()).collect())
            .unwrap_or_default(),
        VariableEntity::Variable => shown
            .map(|id| {
                vars.variables
                    .iter()
                    .filter(|v| v.collection == id)
                    .map(|v| v.name.clone())
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn rename_command(target: &ManageTarget, name: String) -> Command {
    match target {
        ManageTarget::Collection { id } => Command::RenameCollection {
            id: id.clone(),
            name,
        },
        ManageTarget::Mode { collection, id } => Command::RenameMode {
            collection: collection.clone(),
            mode: id.clone(),
            name,
        },
        ManageTarget::Variable { id } => Command::RenameVariable {
            id: id.clone(),
            name,
        },
    }
}

impl App {
    /// An action of the manage view that changes the view only.
    pub(crate) fn manage_view(&mut self, op: ManageOp) {
        let vars = self.replica.vars();
        let nodes = self.replica.nodes();
        let m = &mut self.state.manage;
        match op {
            ManageOp::Choose { id } => {
                if vars.collection(&id).is_none() {
                    return drop(refuse(&mut self.events, "E_COLLECTION_NOT_FOUND"));
                }
                m.chosen = Some(id);
                m.removal = None;
                m.rename = None;
            }
            ManageOp::SetDraft { form, text } => {
                let draft = match form {
                    VariableEntity::Collection => &mut m.drafts.collection,
                    VariableEntity::Mode => &mut m.drafts.mode,
                    VariableEntity::Variable => &mut m.drafts.variable,
                };
                draft.text = kept(text, m.limits.name_max);
            }
            ManageOp::AskRemove { target } => match removal_of(nodes, vars, &target) {
                Ok(removal) => {
                    m.removal = Some(removal);
                    m.rename = None;
                }
                Err(code) => drop(refuse(&mut self.events, code)),
            },
            ManageOp::CancelRemove {} => m.removal = None,
            ManageOp::StartRename { target } => match rename_context(vars, &target) {
                Some((current, _)) => {
                    let current = label(&current, MAX_LABEL);
                    let first = match &target {
                        ManageTarget::Mode { collection, id } => vars
                            .collection(collection)
                            .is_some_and(|c| c.modes.first().is_some_and(|f| f.id == *id)),
                        _ => false,
                    };
                    m.removal = None;
                    m.rename = Some(RenameField {
                        target,
                        opened: current.clone(),
                        text: current.clone(),
                        current,
                        check: Default::default(),
                        unchanged: true,
                        first,
                    });
                }
                None => {
                    let code = gone_code(vars, &target);
                    drop(refuse(&mut self.events, code));
                }
            },
            ManageOp::SetRenameText { text } => {
                if let Some(r) = &mut m.rename {
                    r.text = kept(text, m.limits.name_max);
                }
            }
            ManageOp::CancelRename {} => m.rename = None,
            ManageOp::Close {} => {
                m.chosen = None;
                m.removal = None;
                m.rename = None;
                m.drafts = ManageDrafts::default();
            }
            // The two that change the file are the document's (`document`).
            ManageOp::ConfirmRemove {} | ManageOp::SubmitRename {} => {}
        }
    }

    /// Save the name in the rename field.
    pub(crate) fn submit_rename(&mut self) -> Result<(), SubmitError> {
        let Some(field) = self.state.manage.rename.clone() else {
            return Ok(());
        };
        if let Some(error) = field.check.error {
            // Each says its own reason: the core's code for a name that is not a path, a duplicate or too long, and the
            // view's for a name that is empty or hides something (the core would take those as text).
            let code = match error {
                NameError::Empty => "name_empty",
                NameError::Hidden => "name_hidden",
                NameError::NotPath => "E_INVALID_NAME",
                NameError::Duplicate => "E_DUPLICATE_NAME",
                NameError::TooLong => "E_TOO_LONG",
            };
            return refuse(&mut self.events, code);
        }
        if field.unchanged {
            self.state.manage.rename = None;
            return Ok(());
        }
        let command = rename_command(&field.target, field.check.name.clone());
        self.edit_variables(&[command])?;
        self.state.manage.rename = None;
        self.events.push(Event::Renamed {
            entity: field.target.entity(),
            from: field.current,
            to: label(&field.check.name, MAX_LABEL),
        });
        Ok(())
    }

    /// Go ahead with the removal the question asks about; the question closes either way.
    pub(crate) fn confirm_remove(&mut self) -> Result<(), SubmitError> {
        let Some(r) = self.state.manage.removal.take() else {
            return Ok(());
        };
        match r.target {
            ManageTarget::Variable { id } => self.remove_entity(Target::Variable(&id), r.detach),
            ManageTarget::Collection { id } => {
                self.remove_entity(Target::Collection(&id), r.detach)
            }
            ManageTarget::Mode { collection, id } => self.remove_mode(collection, id),
        }
    }

    /// A collection, a mode or a variable was made by the manage actions: the form that made it starts again, and a new
    /// collection is the one shown, which ends a question and a rename field (they were about the collection before).
    pub(crate) fn manage_added(&mut self, entity: VariableEntity, id: &str) {
        let m = &mut self.state.manage;
        match entity {
            VariableEntity::Collection => {
                m.drafts.collection = NameDraft::default();
                m.chosen = Some(id.to_string());
                m.removal = None;
                m.rename = None;
            }
            VariableEntity::Mode => m.drafts.mode = NameDraft::default(),
            VariableEntity::Variable => m.drafts.variable = NameDraft::default(),
        }
    }

    /// What follows the file: the shown collection, the checks of the forms, the numbers of a question (it ends with its
    /// target), and the field of a rename (it ends with its target, and says what is gone; its checks follow the names).
    pub(crate) fn refresh_manage(&mut self) {
        let vars = self.replica.vars();
        let nodes = self.replica.nodes();
        let m = &mut self.state.manage;
        if m.chosen
            .as_ref()
            .is_some_and(|id| vars.collection(id).is_none())
        {
            m.chosen = None;
        }
        let at = shown(vars, m.chosen.as_ref());
        let at_id = at.map(str::to_string);
        if m.shown != at_id {
            m.shown = at_id;
        }
        let drafts = ManageDrafts {
            collection: draft(vars, at, VariableEntity::Collection, &m.drafts.collection),
            mode: draft(vars, at, VariableEntity::Mode, &m.drafts.mode),
            variable: draft(vars, at, VariableEntity::Variable, &m.drafts.variable),
        };
        if drafts != m.drafts {
            m.drafts = drafts;
        }
        if let Some(r) = &m.removal {
            match removal_of(nodes, vars, &r.target) {
                Ok(next) if next != *r => m.removal = Some(next),
                Ok(_) => {}
                // Another actor removed it: the question has nothing to ask about.
                Err(_) => m.removal = None,
            }
        }
        let Some(field) = m.rename.clone() else {
            return;
        };
        let Some((current, others)) = rename_context(vars, &field.target) else {
            m.rename = None;
            let code = gone_code(vars, &field.target);
            return drop(refuse(&mut self.events, code));
        };
        let current = label(&current, MAX_LABEL);
        let others: Vec<String> = others.iter().map(|o| label(o, MAX_LABEL)).collect();
        let (check, unchanged) = check_rename(
            field.target.entity(),
            &field.text,
            &current,
            &field.opened,
            &others,
        );
        let next = RenameField {
            current,
            check,
            unchanged,
            ..field
        };
        if m.rename.as_ref() != Some(&next) {
            m.rename = Some(next);
        }
    }
}

/// A form's draft with its check made against the names it is compared with.
fn draft(
    vars: &Variables,
    shown: Option<&str>,
    form: VariableEntity,
    old: &NameDraft,
) -> NameDraft {
    // Nothing typed: the check is the same whatever the names are, so the file is not read (this runs on every dispatch).
    if old.text.is_empty() {
        return NameDraft {
            text: String::new(),
            check: check_name(form, "", &[]),
        };
    }
    let names = existing(vars, shown, form);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    NameDraft {
        text: old.text.clone(),
        check: check_name(form, &old.text, &names),
    }
}
