//! The rules of `History::rewrite` for variable commands, per command in my history against the remote
//! command `r` (which is ordered after it). The unit of last writer wins is `(variable, mode)`, as `(node, field)` is
//! for nodes:
//! - `SetVariableValue` of a value the remote command also set, or of a variable it removed or made again,
//!   goes.
//! - `AddVariable` (the undo of a remove) and `AddCollection` take the other actor's writes into the snapshot,
//!   as `AddNode` does: a remote value, a mode added or removed. A variable or collection that the remote
//!   command made again goes.
//! - `RemoveVariable`, `RemoveCollection`, `AddMode` and `RemoveMode` go when the remote command did the same.

use crate::remote::RemoteCtx;
use meldframe_protocol::{Bind, Command, Mode, Value, Variable};

/// Rewrite the variable command `c` against `r`; what stays is pushed on `list`.
pub(crate) fn xform_var(c: Command, r: &Command, ctx: &RemoteCtx, list: &mut Vec<Command>) {
    match c {
        Command::SetVariableValue { id, mode, value } => {
            let gone = match r {
                Command::SetVariableValue {
                    id: rid, mode: rm, ..
                } => *rid == id && *rm == mode,
                Command::RemoveVariable { id: rid } => *rid == id,
                Command::AddVariable { variable } => variable.id == id,
                Command::RemoveMode { mode: rm, .. } => *rm == mode,
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
