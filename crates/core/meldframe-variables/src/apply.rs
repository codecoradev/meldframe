use meldframe_protocol::{Command, Mode, Variables};

/// Whether `cmd` is a variable command (it changes the variables, not the nodes).
pub fn is_variable_command(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::AddCollection { .. }
            | Command::RemoveCollection { .. }
            | Command::AddMode { .. }
            | Command::RemoveMode { .. }
            | Command::AddVariable { .. }
            | Command::RemoveVariable { .. }
            | Command::SetVariableValue { .. }
    )
}

/// Apply a variable command that `meldframe-check` accepted. A node command does nothing here.
pub fn apply(vars: &mut Variables, cmd: &Command) {
    match cmd {
        Command::AddCollection { collection } => vars.collections.push(collection.clone()),
        Command::RemoveCollection { id } => vars.collections.retain(|c| c.id != *id),
        Command::AddMode {
            collection,
            mode,
            index,
            copy_from,
        } => {
            let Some(col) = vars.collections.iter_mut().find(|c| c.id == *collection) else {
                return;
            };
            let from = copy_from
                .clone()
                .or_else(|| col.modes.first().map(|m| m.id.clone()));
            let at = index.unwrap_or(col.modes.len()).min(col.modes.len());
            col.modes.insert(at, mode.clone());
            for v in vars
                .variables
                .iter_mut()
                .filter(|v| v.collection == *collection)
            {
                if let Some(copy) = from.as_ref().and_then(|f| v.values.get(f)).cloned() {
                    v.values.insert(mode.id.clone(), copy);
                }
            }
        }
        Command::RemoveMode { collection, mode } => {
            if let Some(col) = vars.collections.iter_mut().find(|c| c.id == *collection) {
                col.modes.retain(|m| m.id != *mode);
            }
            for v in vars
                .variables
                .iter_mut()
                .filter(|v| v.collection == *collection)
            {
                v.values.remove(mode);
            }
        }
        Command::AddVariable { variable } => vars.variables.push(variable.clone()),
        Command::RemoveVariable { id } => vars.variables.retain(|v| v.id != *id),
        Command::SetVariableValue { id, mode, value } => {
            if let Some(v) = vars.variables.iter_mut().find(|v| v.id == *id) {
                v.values.insert(mode.clone(), value.clone());
            }
        }
        _ => {}
    }
}

/// The commands that undo variable command `cmd` against `vars`, the variables before it is applied (`cmd`
/// must be accepted). Applied in order they put the variables back exactly. A node command has none.
pub fn invert(vars: &Variables, cmd: &Command) -> Vec<Command> {
    match cmd {
        Command::AddCollection { collection } => vec![Command::RemoveCollection {
            id: collection.id.clone(),
        }],
        Command::RemoveCollection { id } => vars
            .collection(id)
            .map(|c| {
                vec![Command::AddCollection {
                    collection: c.clone(),
                }]
            })
            .unwrap_or_default(),
        Command::AddMode {
            collection, mode, ..
        } => vec![Command::RemoveMode {
            collection: collection.clone(),
            mode: mode.id.clone(),
        }],
        Command::RemoveMode { collection, mode } => restore_mode(vars, collection, mode),
        Command::AddVariable { variable } => vec![Command::RemoveVariable {
            id: variable.id.clone(),
        }],
        Command::RemoveVariable { id } => vars
            .variable(id)
            .map(|v| {
                vec![Command::AddVariable {
                    variable: v.clone(),
                }]
            })
            .unwrap_or_default(),
        Command::SetVariableValue { id, mode, .. } => vars
            .variable(id)
            .and_then(|v| v.values.get(mode))
            .map(|old| {
                vec![Command::SetVariableValue {
                    id: id.clone(),
                    mode: mode.clone(),
                    value: old.clone(),
                }]
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Add the mode back where it was, then set every value it held.
fn restore_mode(vars: &Variables, collection: &str, mode: &str) -> Vec<Command> {
    let Some(col) = vars.collection(collection) else {
        return Vec::new();
    };
    let Some(at) = col.modes.iter().position(|m| m.id == mode) else {
        return Vec::new();
    };
    let m: Mode = col.modes[at].clone();
    let mut out = vec![Command::AddMode {
        collection: collection.to_string(),
        mode: m,
        index: Some(at),
        copy_from: None,
    }];
    for v in vars.variables.iter().filter(|v| v.collection == collection) {
        if let Some(old) = v.values.get(mode) {
            out.push(Command::SetVariableValue {
                id: v.id.clone(),
                mode: mode.to_string(),
                value: old.clone(),
            });
        }
    }
    out
}

/// The ids a variable command touches, for `ifSeq`: `$var:<id>` for a variable and `$col:<id>` for a
/// collection (a node id cannot hold a `$`, so they never clash). A node command touches none.
pub fn touched(cmd: &Command) -> Vec<String> {
    let var = |id: &str| format!("$var:{id}");
    let col = |id: &str| format!("$col:{id}");
    match cmd {
        Command::AddCollection { collection } => vec![col(&collection.id)],
        Command::RemoveCollection { id } => vec![col(id)],
        Command::AddMode { collection, .. } | Command::RemoveMode { collection, .. } => {
            vec![col(collection)]
        }
        Command::AddVariable { variable } => vec![var(&variable.id)],
        Command::RemoveVariable { id } | Command::SetVariableValue { id, .. } => vec![var(id)],
        _ => Vec::new(),
    }
}
