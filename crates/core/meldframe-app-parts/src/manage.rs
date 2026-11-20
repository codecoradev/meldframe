//! The commands of the Variables manage actions: what a new collection, mode or variable is, and
//! what a removal comes to when the caller asks to let go of what uses the variable first. They read no `App`: ids are
//! given (the app mints them), the file is an argument, and the answer is a list of commands or the code of a refusal.

use meldframe_protocol::{
    Bind, Collection, Command, DEFAULT_FILL, Mode, Node, NodePatch, Value, VarType, Variable,
    Variables,
};
use meldframe_render::Fields;
use meldframe_variables::{ModeChoice, VarError, node_bindings, resolve};
use std::collections::{BTreeMap, HashSet};

/// The name of the one mode of a new collection, as the import names the mode of a theme with no modes.
pub const FIRST_MODE: &str = "default";

/// What a variable holds when it is made: a colour is the default fill of a node, a number is 0.
pub fn neutral(kind: VarType) -> Value {
    match kind {
        VarType::Color => Value::Color(DEFAULT_FILL.to_string()),
        VarType::Number => Value::Number(0.0),
    }
}

/// A collection with one mode, the default one.
pub fn new_collection(id: &str, mode_id: &str, name: &str) -> Command {
    Command::AddCollection {
        collection: Collection {
            id: id.to_string(),
            name: name.to_string(),
            modes: vec![Mode {
                id: mode_id.to_string(),
                name: FIRST_MODE.to_string(),
            }],
        },
    }
}

/// A mode after the last one of `collection`, with the values of `copy_from` (none: the default mode).
pub fn new_mode(collection: &str, id: &str, name: &str, copy_from: Option<String>) -> Command {
    Command::AddMode {
        collection: collection.to_string(),
        mode: Mode {
            id: id.to_string(),
            name: name.to_string(),
        },
        index: None,
        copy_from,
    }
}

/// A variable with the neutral value of its type in every mode of its collection; the code of the refusal when the
/// collection is not there.
pub fn new_variable(
    vars: &Variables,
    id: &str,
    collection: &str,
    name: &str,
    kind: VarType,
) -> Result<Command, &'static str> {
    let Some(col) = vars.collection(collection) else {
        return Err(VarError::CollectionNotFound(collection.to_string()).code());
    };
    let values: BTreeMap<String, Bind<Value>> = col
        .modes
        .iter()
        .map(|m| (m.id.clone(), Bind::Lit(neutral(kind))))
        .collect();
    Ok(Command::AddVariable {
        variable: Variable {
            id: id.to_string(),
            collection: collection.to_string(),
            name: name.to_string(),
            kind,
            values,
        },
    })
}

/// What a removal is about.
#[derive(Debug, Clone, Copy)]
pub enum Target<'a> {
    Variable(&'a str),
    Collection(&'a str),
}

/// The commands of a removal. Without `detach` it is the one removal command, and the core refuses what is still
/// used. With `detach` it is first what lets go of every user (the node fields bound to a variable that goes, and the
/// aliases of it from a variable that stays) and then the removals, so the whole list is one undo step. The code of the
/// refusal when the variable or the collection is not there.
///
/// The literal a node field keeps is the one it paints with every collection in its default mode (docs/tokens-and-variables.md
/// 5.2). An alias keeps the value it has in its own mode, with the other collections in their default modes: in the
/// default mode that is the same literal, and in another mode the alias does not jump to the value of the default one.
pub fn removal(
    nodes: &[Node],
    vars: &Variables,
    target: Target<'_>,
    detach: bool,
) -> Result<Vec<Command>, &'static str> {
    let doomed: Vec<&Variable> = match target {
        Target::Variable(id) => {
            let Some(v) = vars.variable(id) else {
                return Err(VarError::VariableNotFound(id.to_string()).code());
            };
            vec![v]
        }
        Target::Collection(id) => {
            if vars.collection(id).is_none() {
                return Err(VarError::CollectionNotFound(id.to_string()).code());
            }
            vars.variables
                .iter()
                .filter(|v| v.collection == id)
                .collect()
        }
    };
    let last = match target {
        Target::Variable(_) => None,
        Target::Collection(id) => Some(Command::RemoveCollection { id: id.to_string() }),
    };
    if !detach {
        return Ok(vec![match target {
            Target::Variable(id) => Command::RemoveVariable { id: id.to_string() },
            Target::Collection(_) => last.expect("a collection"),
        }]);
    }
    let ids: HashSet<&str> = doomed.iter().map(|v| v.id.as_str()).collect();
    let mut commands = unbind_nodes(nodes, vars, &ids);
    commands.extend(unalias(vars, &ids));
    commands.extend(removal_order(&doomed));
    commands.extend(last);
    Ok(commands)
}

/// One `UpdateNode` for each node with a field bound to a variable of `ids`: the field is the literal it paints with
/// every collection in its default mode.
fn unbind_nodes(nodes: &[Node], vars: &Variables, ids: &HashSet<&str>) -> Vec<Command> {
    let choice = ModeChoice::default();
    let fields = Fields::new(vars, &choice);
    nodes
        .iter()
        .filter_map(|n| {
            let mut patch = NodePatch::default();
            let mut any = false;
            for b in node_bindings(n).into_iter().filter(|b| ids.contains(b.var)) {
                any = true;
                match b.field {
                    "fill" => patch.fill = Some(Bind::Lit(fields.fill_text(&n.fill))),
                    "radius" => patch.radius = Some(Bind::Lit(fields.radius(&n.radius))),
                    _ => patch.opacity = Some(Bind::Lit(fields.opacity(&n.opacity))),
                }
            }
            any.then(|| Command::UpdateNode {
                id: n.id.clone(),
                patch,
            })
        })
        .collect()
}

/// A `SetVariableValue` with the resolved literal for each value of a variable that stays and aliases one of `ids`.
/// A value that does not resolve (not in a file that loaded) is left, and the removal is then refused as in use.
fn unalias(vars: &Variables, ids: &HashSet<&str>) -> Vec<Command> {
    let mut out = Vec::new();
    for v in vars
        .variables
        .iter()
        .filter(|v| !ids.contains(v.id.as_str()))
    {
        for (mode, value) in &v.values {
            if !value.var_id().is_some_and(|t| ids.contains(t)) {
                continue;
            }
            let mut choice = ModeChoice::default();
            choice.set(&v.collection, mode);
            if let Some(lit) = resolve(vars, &v.id, &choice) {
                out.push(Command::SetVariableValue {
                    id: v.id.clone(),
                    mode: mode.clone(),
                    value: Bind::Lit(lit),
                });
            }
        }
    }
    out
}

/// `RemoveVariable` for each of `doomed`, a variable before the ones it aliases, so none is removed while another that
/// goes still points at it.
fn removal_order(doomed: &[&Variable]) -> Vec<Command> {
    let remove = |v: &&Variable| Command::RemoveVariable { id: v.id.clone() };
    let mut rest: Vec<&Variable> = doomed.to_vec();
    let mut out = Vec::with_capacity(rest.len());
    while !rest.is_empty() {
        let pointed: HashSet<&str> = rest
            .iter()
            .flat_map(|v| v.values.values().filter_map(|b| b.var_id()))
            .collect();
        let (free, held): (Vec<&Variable>, Vec<&Variable>) =
            rest.iter().partition(|v| !pointed.contains(v.id.as_str()));
        if free.is_empty() {
            // An alias cycle cannot be in a file that loaded; the core refuses the rest if there were one.
            out.extend(held.iter().map(remove));
            break;
        }
        out.extend(free.iter().map(remove));
        rest = held;
    }
    out
}
