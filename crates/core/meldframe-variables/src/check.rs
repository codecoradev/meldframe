//! Checks of variable commands, of the bindings of a node and of the variables of a stored file. Nothing here
//! changes the file; the rules that cannot be seen in one value (an alias cycle, a name that is taken, a
//! variable that is still used) read the variables and the nodes the command would meet.

use crate::error::VarError;
use crate::limits::VarLimits;
use crate::{Aliases, Binding, MAX_ALIAS_DEPTH, node_bindings, patch_bindings, users};
use meldframe_protocol::{
    Bind, Collection, Command, Node, NodePatch, Value, VarType, Variable, Variables,
};
use std::collections::HashSet;

fn common(code: &'static str, message: String) -> VarError {
    VarError::Common { code, message }
}

fn strlen(field: &str, s: &str, max: usize) -> Result<(), VarError> {
    if s.chars().count() > max {
        return Err(common(
            "E_TOO_LONG",
            format!("{field} exceeds {max} characters"),
        ));
    }
    Ok(())
}

fn too_many(field: &str, max: usize) -> VarError {
    common(
        "E_TOO_MANY_ENTRIES",
        format!("{field} has too many entries (max {max})"),
    )
}

fn duplicate_id(id: &str) -> VarError {
    common("E_DUPLICATE_ID", format!("id already in use: {id}"))
}

/// A hex colour: `#` and 3, 4, 6 or 8 hexadecimal digits.
fn color(field: &str, s: &str) -> Result<(), VarError> {
    strlen(field, s, 64)?;
    let ok = s.strip_prefix('#').is_some_and(|h| {
        matches!(h.len(), 3 | 4 | 6 | 8) && h.bytes().all(|b| b.is_ascii_hexdigit())
    });
    if ok {
        Ok(())
    } else {
        Err(common(
            "E_INVALID_COLOR",
            format!("{field} must be a hex colour (#rgb, #rgba, #rrggbb or #rrggbbaa)"),
        ))
    }
}

fn number(field: &str, v: f64, max: f64) -> Result<(), VarError> {
    if !v.is_finite() {
        return Err(common(
            "E_NOT_FINITE",
            format!("{field} is not a finite number"),
        ));
    }
    if v.abs() > max {
        return Err(common(
            "E_OUT_OF_RANGE",
            format!("{field} is out of range: {v}"),
        ));
    }
    Ok(())
}

/// An id of letters, digits and `_-:.`.
fn valid_id(id: &str, l: &VarLimits) -> Result<(), VarError> {
    let ok = !id.is_empty()
        && id.chars().count() <= l.max_id
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '.'));
    if ok {
        Ok(())
    } else {
        Err(common("E_INVALID_ID", format!("invalid id: {id:?}")))
    }
}

/// A variable or collection name: lowercase segments of letters, digits and single hyphens, joined by `/`.
fn valid_name(name: &str, l: &VarLimits) -> Result<(), VarError> {
    strlen("name", name, l.max_name)?;
    let segment = |s: &str| {
        !s.is_empty()
            && !s.starts_with('-')
            && !s.ends_with('-')
            && !s.contains("--")
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    if name.split('/').all(segment) {
        Ok(())
    } else {
        Err(VarError::InvalidName(name.to_string()))
    }
}

/// The name of a mode: any text up to the length limit, as `addMode` takes it.
fn mode_name(name: &str, l: &VarLimits) -> Result<(), VarError> {
    strlen("mode name", name, l.max_name)
}

fn collection_of<'a>(vars: &'a Variables, id: &str) -> Result<&'a Collection, VarError> {
    vars.collection(id)
        .ok_or_else(|| VarError::CollectionNotFound(id.to_string()))
}

fn variable_of<'a>(vars: &'a Variables, id: &str) -> Result<&'a Variable, VarError> {
    vars.variable(id)
        .ok_or_else(|| VarError::VariableNotFound(id.to_string()))
}

/// The bound fields of a node or patch must name a variable that exists and has the type of the field.
fn check_bindings(vars: &Variables, bindings: Vec<Binding<'_>>) -> Result<(), VarError> {
    for b in bindings {
        let v = variable_of(vars, b.var)?;
        if v.kind != b.expects {
            return Err(VarError::TypeMismatch {
                field: b.field.to_string(),
                expected: b.expects,
                got: v.kind,
            });
        }
    }
    Ok(())
}

/// The bindings of a node being added.
pub fn check_node_bindings(vars: &Variables, n: &Node) -> Result<(), VarError> {
    check_bindings(vars, node_bindings(n))
}

/// The bindings of a patch.
pub fn check_patch_bindings(vars: &Variables, p: &NodePatch) -> Result<(), VarError> {
    check_bindings(vars, patch_bindings(p))
}

/// One value of variable `owner` in mode `mode`: a literal of its type, or an alias to a variable of the same
/// type that closes no cycle and keeps every chain within [`MAX_ALIAS_DEPTH`] steps. `vars` already holds
/// the alias when the file is loaded and does not yet when a command is checked; the answer is the same.
fn check_value(
    aliases: &Aliases<'_>,
    loaded: bool,
    owner: (&str, VarType),
    mode: &str,
    value: &Bind<Value>,
    l: &VarLimits,
) -> Result<(), VarError> {
    let field = format!("values.{mode}");
    match value {
        Bind::Lit(v) => {
            if v.var_type() != owner.1 {
                return Err(VarError::TypeMismatch {
                    field,
                    expected: owner.1,
                    got: v.var_type(),
                });
            }
            match v {
                Value::Color(s) => color("value", s),
                Value::Number(n) => number("value", *n, l.max_size),
            }
        }
        Bind::Var(r) => {
            let target = aliases
                .variable(&r.var)
                .ok_or_else(|| VarError::VariableNotFound(r.var.clone()))?;
            if target.kind != owner.1 {
                return Err(VarError::TypeMismatch {
                    field,
                    expected: owner.1,
                    got: target.kind,
                });
            }
            // A stored file holds the alias already, so a cycle shows as the owner reaching one; a command
            // is checked before its alias is in, so the question is whether the target leads back.
            let cycle = if loaded {
                aliases.reaches_a_cycle(owner.0)
            } else {
                aliases.reaches(&target.id, owner.0)
            };
            if cycle {
                return Err(VarError::AliasCycle(owner.0.to_string()));
            }
            let chain = aliases
                .height(owner.0)
                .saturating_add(1)
                .saturating_add(aliases.depth(&target.id));
            if chain > MAX_ALIAS_DEPTH {
                return Err(VarError::AliasTooDeep(MAX_ALIAS_DEPTH));
            }
            Ok(())
        }
    }
}

/// A variable of collection `col` as `AddVariable` or a stored file holds it: its name, one value for each
/// mode, each value valid.
fn check_variable(
    aliases: &Aliases<'_>,
    loaded: bool,
    col: &Collection,
    v: &Variable,
    l: &VarLimits,
) -> Result<(), VarError> {
    valid_name(&v.name, l)?;
    let modes: HashSet<&str> = col.modes.iter().map(|m| m.id.as_str()).collect();
    if v.values.len() != modes.len() || !v.values.keys().all(|k| modes.contains(k.as_str())) {
        return Err(VarError::IncompleteValues(v.id.clone()));
    }
    for (mode, value) in &v.values {
        check_value(aliases, loaded, (&v.id, v.kind), mode, value, l)?;
    }
    Ok(())
}

fn check_modes(c: &Collection, l: &VarLimits) -> Result<(), VarError> {
    if c.modes.is_empty() {
        return Err(VarError::InvalidModes(
            "a collection needs at least one mode",
        ));
    }
    if c.modes.len() > l.max_modes {
        return Err(too_many("modes", l.max_modes));
    }
    let mut seen = HashSet::new();
    for m in &c.modes {
        valid_id(&m.id, l)?;
        mode_name(&m.name, l)?;
        if !seen.insert(m.id.as_str()) {
            return Err(VarError::InvalidModes("two modes have the same id"));
        }
    }
    Ok(())
}

fn check_collection(vars: &Variables, c: &Collection, l: &VarLimits) -> Result<(), VarError> {
    valid_id(&c.id, l)?;
    if vars.collection(&c.id).is_some() {
        return Err(duplicate_id(&c.id));
    }
    valid_name(&c.name, l)?;
    check_modes(c, l)
}

/// The names of what still uses variable `id`: nodes, then variables that alias it (at most 20).
fn users_of(nodes: &[Node], vars: &Variables, id: &str) -> Vec<String> {
    let (aliases, nodes) = users(nodes, vars, id);
    nodes.into_iter().chain(aliases).take(20).collect()
}

/// Validate a variable command against the file `nodes` and `vars` without changing them.
pub fn check_variable_command(
    nodes: &[Node],
    vars: &Variables,
    cmd: &Command,
    l: &VarLimits,
) -> Result<(), VarError> {
    match cmd {
        Command::AddCollection { collection } => {
            if vars.collections.len() >= l.max_collections {
                return Err(too_many("collections", l.max_collections));
            }
            check_collection(vars, collection, l)
        }
        Command::RemoveCollection { id } => {
            collection_of(vars, id)?;
            if vars.variables.iter().any(|v| v.collection == *id) {
                return Err(VarError::CollectionNotEmpty(id.clone()));
            }
            Ok(())
        }
        // A rename is refused for a name `addCollection`, `addMode` or `addVariable` would refuse for the same
        // entity, and for a name another variable of the collection has. The name it already has is accepted
        // (and changes nothing), so a repeated rename is harmless.
        Command::RenameCollection { id, name } => {
            collection_of(vars, id)?;
            valid_name(name, l)
        }
        Command::RenameMode {
            collection,
            mode,
            name,
        } => {
            let col = collection_of(vars, collection)?;
            if !col.modes.iter().any(|m| m.id == *mode) {
                return Err(VarError::ModeNotFound(mode.clone()));
            }
            mode_name(name, l)
        }
        Command::RenameVariable { id, name } => {
            let v = variable_of(vars, id)?;
            valid_name(name, l)?;
            if vars
                .variables
                .iter()
                .any(|o| o.id != *id && o.collection == v.collection && o.name == *name)
            {
                return Err(VarError::DuplicateName(name.clone()));
            }
            Ok(())
        }
        Command::AddMode {
            collection,
            mode,
            index,
            copy_from,
        } => {
            let col = collection_of(vars, collection)?;
            if let Some(from) = copy_from
                && !col.modes.iter().any(|m| m.id == *from)
            {
                return Err(VarError::ModeNotFound(from.clone()));
            }
            let mut grown = col.clone();
            grown.modes.push(mode.clone());
            check_modes(&grown, l)?;
            match index {
                Some(0) => Err(VarError::DefaultMode(mode.id.clone())),
                Some(i) if *i > col.modes.len() => Err(common(
                    "E_OUT_OF_RANGE",
                    format!("index is out of range: {i}"),
                )),
                _ => Ok(()),
            }
        }
        Command::RemoveMode { collection, mode } => {
            let col = collection_of(vars, collection)?;
            let at = col
                .modes
                .iter()
                .position(|m| m.id == *mode)
                .ok_or_else(|| VarError::ModeNotFound(mode.clone()))?;
            if at == 0 {
                return Err(VarError::DefaultMode(mode.clone()));
            }
            Ok(())
        }
        Command::AddVariable { variable } => {
            valid_id(&variable.id, l)?;
            if vars.variable(&variable.id).is_some() {
                return Err(duplicate_id(&variable.id));
            }
            let col = collection_of(vars, &variable.collection)?;
            if vars.variables.len() >= l.max_variables {
                return Err(too_many("variables", l.max_variables));
            }
            if vars
                .variables
                .iter()
                .any(|v| v.collection == col.id && v.name == variable.name)
            {
                return Err(VarError::DuplicateName(variable.name.clone()));
            }
            check_variable(&Aliases::new(vars), false, col, variable, l)
        }
        Command::RemoveVariable { id } => {
            variable_of(vars, id)?;
            let by = users_of(nodes, vars, id);
            if by.is_empty() {
                Ok(())
            } else {
                Err(VarError::VariableInUse { id: id.clone(), by })
            }
        }
        Command::SetVariableValue { id, mode, value } => {
            let v = variable_of(vars, id)?;
            let col = collection_of(vars, &v.collection)?;
            if !col.modes.iter().any(|m| m.id == *mode) {
                return Err(VarError::ModeNotFound(mode.clone()));
            }
            check_value(&Aliases::new(vars), false, (&v.id, v.kind), mode, value, l)
        }
        _ => Ok(()),
    }
}

/// Check the variables of a stored file as a whole: unique ids, valid names, unique names per collection,
/// complete and valid values, no alias cycle and no chain that is too long. The first problem is returned
/// with the id of the variable or collection it is about (empty for the whole set).
pub fn check_variables(vars: &Variables, l: &VarLimits) -> Result<(), (String, VarError)> {
    let at = |id: &str, e: VarError| (id.to_string(), e);
    if vars.collections.len() > l.max_collections {
        return Err(at("", too_many("collections", l.max_collections)));
    }
    if vars.variables.len() > l.max_variables {
        return Err(at("", too_many("variables", l.max_variables)));
    }
    let mut ids = HashSet::new();
    for c in &vars.collections {
        valid_id(&c.id, l).map_err(|e| at(&c.id, e))?;
        if !ids.insert(c.id.as_str()) {
            return Err(at(&c.id, duplicate_id(&c.id)));
        }
        valid_name(&c.name, l)
            .and_then(|()| check_modes(c, l))
            .map_err(|e| at(&c.id, e))?;
    }
    let aliases = Aliases::new(vars);
    let mut var_ids = HashSet::new();
    let mut names = HashSet::new();
    for v in &vars.variables {
        valid_id(&v.id, l).map_err(|e| at(&v.id, e))?;
        if !var_ids.insert(v.id.as_str()) {
            return Err(at(&v.id, duplicate_id(&v.id)));
        }
        if !names.insert((v.collection.as_str(), v.name.as_str())) {
            return Err(at(&v.id, VarError::DuplicateName(v.name.clone())));
        }
        let col = collection_of(vars, &v.collection).map_err(|e| at(&v.id, e))?;
        check_variable(&aliases, true, col, v, l).map_err(|e| at(&v.id, e))?;
    }
    Ok(())
}

/// Every binding of the nodes of a stored file names a variable of `vars` of the right type.
pub fn check_file_bindings(nodes: &[Node], vars: &Variables) -> Result<(), (String, VarError)> {
    nodes
        .iter()
        .try_for_each(|n| check_node_bindings(vars, n).map_err(|e| (n.id.clone(), e)))
}
