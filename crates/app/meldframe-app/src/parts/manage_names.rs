//! The rules of the Variables manage view that read the file and no state of the app (audit finding T5): whether a typed
//! name can be sent, what the names it is compared with are, what a rename comes to, and what a removal would touch.
//! The name rule of the core (`meldframe_variables::is_name_path`, the limits of `meldframe-check`) is asked here, so the
//! front end writes none of it. The actions and the state that hold these are in `meldframe-app` (`manage_ui`).

use super::variables_panel::use_counts;
use meldframe_app_model::{
    MODE_NAME_MAX, ManageTarget, NameCheck, NameError, NameWarning, Removal, VariableEntity,
};
use meldframe_check::Limits;
use meldframe_protocol::{Node, Variables};
use meldframe_text::{MAX_LABEL, Profile, label, sanitize};
use meldframe_variables::is_name_path;

/// Whether `name` holds a character that changes how a text reads without showing, or a space that is not the plain one.
/// The core takes a name of any text (a mode) or of a lowercase path (a collection, a variable), so this refuses only what
/// would hide a second meaning: what the display profile of `meldframe-text` removes from a name (it keeps a joiner and
/// selector 16 between emoji, so a sun with its selector is a good name), and every white space but ` `.
fn hidden(name: &str) -> bool {
    sanitize(name, Profile::Display) != name || name.chars().any(|c| c != ' ' && c.is_whitespace())
}

/// Whether a typed name can be sent. A variable or collection is a lowercase slash path (the core refuses anything else); a
/// mode is free text of up to `MODE_NAME_MAX` characters; none holds a hidden character. A variable that exists in `existing`
/// (the names of the collection) is an error, the core refuses it; a collection or a mode that exists is only a warning,
/// since the core takes it (the ids differ).
pub fn check_name(entity: VariableEntity, raw: &str, existing: &[&str]) -> NameCheck {
    let name = raw.trim();
    let fail = |error| NameCheck {
        name: name.to_string(),
        error: Some(error),
        warning: None,
    };
    if name.is_empty() {
        return fail(NameError::Empty);
    }
    if hidden(name) {
        return fail(NameError::Hidden);
    }
    let max = if entity == VariableEntity::Mode {
        MODE_NAME_MAX
    } else {
        Limits::default().max_name
    };
    if name.chars().count() > max {
        return fail(NameError::TooLong);
    }
    if entity != VariableEntity::Mode && !is_name_path(name) {
        return fail(NameError::NotPath);
    }
    if entity == VariableEntity::Variable {
        return if existing.contains(&name) {
            fail(NameError::Duplicate)
        } else {
            NameCheck {
                name: name.to_string(),
                ..Default::default()
            }
        };
    }
    let same = existing
        .iter()
        .any(|e| e.trim().to_lowercase() == name.to_lowercase());
    NameCheck {
        name: name.to_string(),
        error: None,
        warning: same.then_some(NameWarning::Same),
    }
}

/// What a rename is checked against: the name the target has now and the names of the others of its kind (the variables of
/// its collection, the collections of the file, the modes of its collection); `None` when the target is not there.
pub fn rename_context(vars: &Variables, target: &ManageTarget) -> Option<(String, Vec<String>)> {
    match target {
        ManageTarget::Variable { id } => {
            let v = vars.variable(id)?;
            let others = vars
                .variables
                .iter()
                .filter(|x| x.collection == v.collection && x.id != *id)
                .map(|x| x.name.clone())
                .collect();
            Some((v.name.clone(), others))
        }
        ManageTarget::Collection { id } => {
            let c = vars.collection(id)?;
            let others = vars
                .collections
                .iter()
                .filter(|x| x.id != *id)
                .map(|x| x.name.clone())
                .collect();
            Some((c.name.clone(), others))
        }
        ManageTarget::Mode { collection, id } => {
            let c = vars.collection(collection)?;
            let m = c.modes.iter().find(|m| m.id == *id)?;
            let others = c
                .modes
                .iter()
                .filter(|x| x.id != *id)
                .map(|x| x.name.clone())
                .collect();
            Some((m.name.clone(), others))
        }
    }
}

/// The code of the refusal for a target that is not there: what is gone is a variable, a collection (also the collection of
/// a mode) or a mode.
pub fn gone_code(vars: &Variables, target: &ManageTarget) -> &'static str {
    match target {
        ManageTarget::Variable { .. } => "E_VARIABLE_NOT_FOUND",
        ManageTarget::Collection { .. } => "E_COLLECTION_NOT_FOUND",
        ManageTarget::Mode { collection, .. } if vars.collection(collection).is_none() => {
            "E_COLLECTION_NOT_FOUND"
        }
        ManageTarget::Mode { .. } => "E_MODE_NOT_FOUND",
    }
}

/// A rename's check: the rules of an addition with the own name left out of the names taken. A text that is the name the
/// target has now, or the one the field opened with, is no change (`true`): saving it closes the field. If another actor
/// renamed the thing meanwhile and the person typed nothing, sending the opened text would undo that rename.
pub fn check_rename(
    entity: VariableEntity,
    raw: &str,
    current: &str,
    opened: &str,
    others: &[String],
) -> (NameCheck, bool) {
    let name = raw.trim();
    if raw == current || name == current || raw == opened || name == opened {
        let check = NameCheck {
            name: name.to_string(),
            ..Default::default()
        };
        return (check, true);
    }
    let others: Vec<&str> = others.iter().map(String::as_str).collect();
    (check_name(entity, raw, &others), false)
}

/// The question before a removal, from the file: the target's name and what removing it would touch. The code of the
/// refusal when the target is not there. The uses are counted on the whole file, whatever a search narrows the panel to.
pub fn removal_of(
    nodes: &[Node],
    vars: &Variables,
    target: &ManageTarget,
) -> Result<Removal, &'static str> {
    let (name, detach, uses, variables) = match target {
        ManageTarget::Variable { id } => {
            let Some(v) = vars.variable(id) else {
                return Err(gone_code(vars, target));
            };
            let uses = use_counts(nodes, vars)
                .get(id.as_str())
                .copied()
                .unwrap_or(0);
            (v.name.clone(), uses > 0, uses, 0)
        }
        ManageTarget::Collection { id } => {
            let Some(c) = vars.collection(id) else {
                return Err(gone_code(vars, target));
            };
            let counts = use_counts(nodes, vars);
            let mine = vars.variables.iter().filter(|v| v.collection == *id);
            let (n, uses) = mine.fold((0, 0), |(n, u), v| {
                (n + 1, u + counts.get(v.id.as_str()).copied().unwrap_or(0))
            });
            (c.name.clone(), n > 0, uses, n)
        }
        ManageTarget::Mode { collection, id } => {
            let m = vars
                .collection(collection)
                .and_then(|c| c.modes.iter().find(|m| m.id == *id));
            let Some(m) = m else {
                return Err(gone_code(vars, target));
            };
            (m.name.clone(), false, 0, 0)
        }
    };
    Ok(Removal {
        target: target.clone(),
        name: label(&name, MAX_LABEL),
        detach,
        uses,
        variables,
    })
}
