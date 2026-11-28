//! The pure helpers of the Variables manage view: the collection shown, a typed text as the state keeps it, the names a new thing
//! is compared with, the command of a rename and a form's draft with its check. They read the variables and what they are given,
//! and hold no state; the transitions that use them are `manage_ui.rs` of `meldframe-app`.

use crate::manage_names::check_name;
use meldframe_app_model::{ManageTarget, NameDraft, VariableEntity};
use meldframe_protocol::{Command, Variables};
use meldframe_text::clip;

/// The collection the view shows: the one chosen, else the first.
pub fn shown<'a>(vars: &'a Variables, chosen: Option<&String>) -> Option<&'a str> {
    chosen
        .and_then(|id| vars.collection(id))
        .or_else(|| vars.collections.first())
        .map(|c| c.id.as_str())
}

/// How many times the longest name a pasted text may be before it is cut: the text is kept so the person can see and cut
/// it, and its check says `TooLong`, but a text of any size is not held in the state.
pub const KEEP_NAMES: usize = 4;

/// A typed text as the state keeps it: whole up to `KEEP_NAMES` times `name_max` characters, cut there beyond.
pub fn kept(text: String, name_max: usize) -> String {
    match clip(&text, KEEP_NAMES * name_max, usize::MAX) {
        c if c.len() == text.len() => text,
        c => c.to_string(),
    }
}

/// The names a new thing of this kind is compared with: the collections of the file, the modes or the variables of the
/// collection that is shown.
pub fn existing(vars: &Variables, shown: Option<&str>, form: VariableEntity) -> Vec<String> {
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

pub fn rename_command(target: &ManageTarget, name: String) -> Command {
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

/// A form's draft with its check made against the names it is compared with.
pub fn draft(
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
