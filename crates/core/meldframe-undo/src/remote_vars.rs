//! The rules of `History::rewrite` for variable commands, per command in my history against the remote
//! command `r` (which is ordered after it). The unit of last writer wins is `(variable, mode)`, as `(node, field)` is
//! for nodes:
//! - `SetVariableValue` of a value the remote command also set, of a variable it removed or made again, or of
//!   a mode it removed from the collection of that variable, goes.
//! - `AddVariable` (the undo of a remove) and `AddCollection` take the other actor's writes into the snapshot,
//!   as `AddNode` does: a remote value, a mode added or removed. A variable or collection that the remote
//!   command made again goes.
//! - `RemoveVariable`, `RemoveCollection`, `AddMode` and `RemoveMode` go when the remote command did the same.
//! - A rename (`RenameVariable`, `RenameCollection`, `RenameMode`) is last writer wins on the name of the entity: it
//!   goes when the remote command renamed, removed or made again the same entity (a mode goes with its collection
//!   too), as a rename of a name that is no longer there would overwrite the other actor. The snapshot of a
//!   variable, a collection or a mode that my undo adds again takes the other actor's rename, as it takes a value.

use crate::remote::RemoteCtx;
use meldframe_protocol::{Bind, Command, Mode, Value, Variable};

/// Whether `r` renamed, removed or made again variable `id`: a rename of it that was built before is stale.
fn variable_replaced(r: &Command, id: &str) -> bool {
    match r {
        Command::RenameVariable { id: rid, .. } | Command::RemoveVariable { id: rid } => rid == id,
        Command::AddVariable { variable } => variable.id == id,
        _ => false,
    }
}

/// The same for collection `id`.
fn collection_replaced(r: &Command, id: &str) -> bool {
    match r {
        Command::RenameCollection { id: rid, .. } | Command::RemoveCollection { id: rid } => {
            rid == id
        }
        Command::AddCollection { collection } => collection.id == id,
        _ => false,
    }
}

/// The same for mode `mode` of collection `collection`; the loss of the collection takes the mode with it.
fn mode_replaced(r: &Command, collection: &str, mode: &str) -> bool {
    match r {
        Command::RenameMode {
            collection: rc,
            mode: rm,
            ..
        }
        | Command::RemoveMode {
            collection: rc,
            mode: rm,
        } => rc == collection && rm == mode,
        Command::AddMode {
            collection: rc,
            mode: rm,
            ..
        } => rc == collection && rm.id == mode,
        Command::RemoveCollection { id } => id == collection,
        Command::AddCollection { collection: c } => c.id == collection,
        _ => false,
    }
}

/// Rewrite the variable command `c` against `r`; what stays is pushed on `list`.
pub(crate) fn xform_var(c: Command, r: &Command, ctx: &RemoteCtx, list: &mut Vec<Command>) {
    match c {
        Command::RenameVariable { id, name } => {
            if !variable_replaced(r, &id) {
                list.push(Command::RenameVariable { id, name });
            }
        }
        Command::RenameCollection { id, name } => {
            if !collection_replaced(r, &id) {
                list.push(Command::RenameCollection { id, name });
            }
        }
        Command::RenameMode {
            collection,
            mode,
            name,
        } => {
            if !mode_replaced(r, &collection, &mode) {
                list.push(Command::RenameMode {
                    collection,
                    mode,
                    name,
                });
            }
        }
        Command::SetVariableValue { id, mode, value } => {
            let gone = match r {
                Command::SetVariableValue {
                    id: rid, mode: rm, ..
                } => *rid == id && *rm == mode,
                Command::RemoveVariable { id: rid } => *rid == id,
                Command::AddVariable { variable } => variable.id == id,
                // Mode ids are per collection: only a variable of the collection that lost the mode loses its
                // value. `mode_vars` is read from the document before the remote command applies; a variable
                // that is not in it (another collection, or unknown to the context, e.g. one my own earlier
                // step in this list adds again, whose `AddVariable` snapshot already drops the mode) keeps
                // its step.
                Command::RemoveMode { mode: rm, .. } => *rm == mode && ctx.mode_vars.contains(&id),
                _ => false,
            };
            if !gone {
                list.push(Command::SetVariableValue { id, mode, value });
            }
        }
        Command::AddVariable { mut variable } => {
            match r {
                Command::AddVariable { variable: rv } if rv.id == variable.id => return,
                Command::SetVariableValue { id, mode, value } if *id == variable.id => {
                    variable.values.insert(mode.clone(), value.clone());
                }
                Command::RenameVariable { id, name } if *id == variable.id => {
                    variable.name = name.clone();
                }
                Command::AddMode {
                    collection,
                    mode,
                    copy_from,
                    ..
                } if *collection == variable.collection => {
                    copy_value(&mut variable, mode, copy_from, ctx)
                }
                Command::RemoveMode { collection, mode } if *collection == variable.collection => {
                    variable.values.remove(mode);
                }
                _ => {}
            }
            list.push(Command::AddVariable { variable });
        }
        Command::RemoveVariable { id } => {
            let done = match r {
                Command::RemoveVariable { id: rid } => *rid == id,
                Command::AddVariable { variable } => variable.id == id,
                _ => false,
            };
            if !done {
                list.push(Command::RemoveVariable { id });
            }
        }
        Command::AddCollection { mut collection } => {
            match r {
                Command::AddCollection { collection: rc } if rc.id == collection.id => return,
                Command::AddMode {
                    collection: rc,
                    mode,
                    index,
                    ..
                } if *rc == collection.id => {
                    let at = index
                        .unwrap_or(collection.modes.len())
                        .min(collection.modes.len());
                    collection.modes.insert(at, mode.clone());
                }
                Command::RemoveMode {
                    collection: rc,
                    mode,
                } if *rc == collection.id => collection.modes.retain(|m| m.id != *mode),
                Command::RenameCollection { id, name } if *id == collection.id => {
                    collection.name = name.clone();
                }
                Command::RenameMode {
                    collection: rc,
                    mode,
                    name,
                } if *rc == collection.id => {
                    if let Some(m) = collection.modes.iter_mut().find(|m| m.id == *mode) {
                        m.name = name.clone();
                    }
                }
                _ => {}
            }
            list.push(Command::AddCollection { collection });
        }
        Command::RemoveCollection { id } => {
            let done = match r {
                Command::RemoveCollection { id: rid } => *rid == id,
                Command::AddCollection { collection } => collection.id == id,
                _ => false,
            };
            if !done {
                list.push(Command::RemoveCollection { id });
            }
        }
        Command::AddMode {
            collection,
            mode,
            index,
            copy_from,
        } => {
            let done = matches!(r, Command::AddMode { collection: rc, mode: rm, .. }
                if *rc == collection && rm.id == mode.id)
                || matches!(r, Command::RemoveCollection { id } if *id == collection);
            // The mode comes back with the name the other actor gave it.
            let mut mode = mode;
            if let Command::RenameMode {
                collection: rc,
                mode: rm,
                name,
            } = r
                && *rc == collection
                && *rm == mode.id
            {
                mode.name = name.clone();
            }
            if !done {
                list.push(Command::AddMode {
                    collection,
                    mode,
                    index,
                    copy_from,
                });
            }
        }
        Command::RemoveMode { collection, mode } => {
            let done = matches!(r, Command::RemoveMode { collection: rc, mode: rm }
                if *rc == collection && *rm == mode)
                || matches!(r, Command::AddMode { collection: rc, mode: rm, .. }
                    if *rc == collection && rm.id == mode);
            if !done {
                list.push(Command::RemoveMode { collection, mode });
            }
        }
        other => list.push(other),
    }
}

/// The snapshot of a variable gets the value of a mode a remote command added: the copy it made, from the mode it
/// named or else from the default mode of the collection (as `apply` does).
fn copy_value(v: &mut Variable, mode: &Mode, copy_from: &Option<String>, ctx: &RemoteCtx) {
    let from: Option<Bind<Value>> = copy_from
        .as_ref()
        .or(ctx.default_mode.as_ref())
        .and_then(|f| v.values.get(f))
        .cloned();
    if let Some(value) = from {
        v.values.insert(mode.id.clone(), value);
    }
}
