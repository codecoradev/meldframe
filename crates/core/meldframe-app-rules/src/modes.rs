//! The modes a collection is shown in and the commands that edit variables, the parts that read no `App`: the mode a
//! pick comes to, the picks a remote change removed, the check of a list of variable commands, and the check of a removal
//! that lets go of what uses a variable first (node updates among the variable commands).

use meldframe_check::{ApplyError, Limits, check_command_with};
use meldframe_protocol::{Command, Node, Variables};
use meldframe_variables::{ModeChoice, VarError, apply, is_variable_command};
use std::collections::BTreeMap;

/// The modes the canvas is painted in, from the picks of the view.
pub fn choice(modes: &BTreeMap<String, String>) -> ModeChoice {
    ModeChoice::of(modes.iter().map(|(c, m)| (c.as_str(), m.as_str())))
}

/// The pick a request comes to: the mode of a collection, or none for its default mode (the default mode is no pick at
/// all, so the state is empty again when every collection is on it); the code of the refusal when the collection or the
/// mode is not there.
pub fn pick(
    vars: &Variables,
    collection: &str,
    mode: Option<String>,
) -> Result<Option<String>, &'static str> {
    let Some(col) = vars.collection(collection) else {
        return Err(VarError::CollectionNotFound(collection.to_string()).code());
    };
    if mode
        .as_ref()
        .is_some_and(|m| !col.modes.iter().any(|x| x.id == *m))
    {
        return Err(VarError::ModeNotFound(mode.unwrap_or_default()).code());
    }
    let first = col.modes.first().map(|m| m.id.clone());
    Ok(mode.filter(|m| Some(m) != first.as_ref()))
}

/// The collections whose pick a remote change removed (the mode, or the collection itself).
pub fn removed(vars: &Variables, modes: &BTreeMap<String, String>) -> Vec<String> {
    modes
        .iter()
        .filter(|(c, m)| {
            !vars
                .collection(c)
                .is_some_and(|col| col.modes.iter().any(|x| x.id == **m))
        })
        .map(|(c, _)| c.clone())
        .collect()
}

/// Why a list of variable commands is not applied.
#[derive(Debug)]
pub enum EditRefusal {
    /// A command of the list is not a variable command.
    NotVariable,
    /// A command is not valid on what the ones before it make of the variables.
    Invalid(ApplyError),
}

/// Check every command on what the ones before it make of the variables.
pub fn check_edit(
    nodes: &[Node],
    vars: &Variables,
    commands: &[Command],
) -> Result<(), EditRefusal> {
    check_edit_at(nodes, vars, commands).map_err(|(_, e)| e)
}

/// [`check_edit`], and the place in the list of the command that is refused, so a caller can name what it is about.
pub fn check_edit_at(
    nodes: &[Node],
    vars: &Variables,
    commands: &[Command],
) -> Result<(), (usize, EditRefusal)> {
    let limits = Limits::default();
    let mut vars = vars.clone();
    for (i, c) in commands.iter().enumerate() {
        if !is_variable_command(c) {
            return Err((i, EditRefusal::NotVariable));
        }
        check_command_with(nodes, &vars, c, &limits).map_err(|e| (i, EditRefusal::Invalid(e)))?;
        apply(&mut vars, c);
    }
    Ok(())
}

/// Check every command on the file as the ones before it leave it. Only a node update and a variable command change the
/// file in this check; the planner of the removals makes no other. The first that fails is the answer, and nothing is
/// applied.
///
/// It has to be `pub`: `meldframe-app` calls it from another crate. For a plan made from a file that loaded it holds by
/// construction, so it is a guard that keeps a plan which would fail late from applying its first half; it is tested
/// directly (`meldframe-variables-manage-tests`, `edges.rs`).
pub fn check_group(
    nodes: &[Node],
    vars: &Variables,
    commands: &[Command],
) -> Result<(), ApplyError> {
    let limits = Limits::default();
    let mut nodes = nodes.to_vec();
    let mut vars = vars.clone();
    for c in commands {
        check_command_with(&nodes, &vars, c, &limits)?;
        match c {
            Command::UpdateNode { id, patch } => {
                if let Some(n) = nodes.iter_mut().find(|n| n.id == *id) {
                    n.apply_patch(patch);
                }
            }
            c if is_variable_command(c) => apply(&mut vars, c),
            _ => {}
        }
    }
    Ok(())
}
