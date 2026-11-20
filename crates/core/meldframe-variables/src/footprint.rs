use meldframe_protocol::Command;

/// `(node, field)` pairs, as the replica's footprint keeps them.
pub type Pairs = Vec<(String, String)>;

/// The field of a rename of a collection or a variable.
const NAME: &str = "$name";

/// The field of the name of mode `id` of a collection.
fn mode_name(id: &str) -> String {
    format!("{NAME}:{id}")
}

/// The footprint of a variable command: what it reads and what it writes, as `(entity, field)` pairs (see
/// `meldframe-sync`). A variable is the pseudo node `$var:<id>` and a collection `$col:<id>` (a node id cannot hold
/// a `$`); a value is the field named by its mode, and the modes of a collection are the field `modes`.
/// A rename writes the field `$name` of the entity (`$name:<mode>` for the name of a mode, in the collection); a `$`
/// cannot be in an id, so no value, mode or collection written under an id can be mistaken for a name. A mode that
/// is added writes its name, so a rename of it, or the undo of a removal of it, depends on it.
/// `exists`, `all` and `whole` are the replica's markers for "the entity exists", "its whole content" and "a
/// write that creates or removes it". A node command has an empty footprint.
pub fn footprint(c: &Command, (exists, all, whole): (&str, &str, &str)) -> (Pairs, Pairs) {
    let pair = |id: String, f: &str| (id, f.to_string());
    let var = |id: &str| format!("$var:{id}");
    let col = |id: &str| format!("$col:{id}");
    // A change of a field reads what it writes, and that the entity exists.
    let field = |entity: String, fields: Vec<String>| {
        let w: Pairs = fields.iter().map(|f| pair(entity.clone(), f)).collect();
        let mut r = w.clone();
        r.push(pair(entity, exists));
        (r, w)
    };
    match c {
        Command::AddCollection { collection } => (
            vec![pair(col(&collection.id), all)],
            vec![pair(col(&collection.id), whole)],
        ),
        Command::RemoveCollection { id } => {
            (vec![pair(col(id), exists)], vec![pair(col(id), whole)])
        }
        Command::RenameCollection { id, .. } => field(col(id), vec![NAME.into()]),
        Command::RenameMode {
            collection, mode, ..
        } => {
            // The mode must exist (its id is a field of the collection, written by the add and the remove).
            let (mut r, w) = field(col(collection), vec![mode_name(mode)]);
            r.push(pair(col(collection), mode));
            (r, w)
        }
        Command::RenameVariable { id, .. } => field(var(id), vec![NAME.into()]),
        Command::AddMode {
            collection, mode, ..
        } => field(
            col(collection),
            vec!["modes".into(), mode.id.clone(), mode_name(&mode.id)],
        ),
        Command::RemoveMode { collection, mode } => {
            field(col(collection), vec!["modes".into(), mode.clone()])
        }
        Command::AddVariable { variable } => (
            vec![
                pair(var(&variable.id), all),
                pair(col(&variable.collection), exists),
            ],
            vec![pair(var(&variable.id), whole)],
        ),
        Command::RemoveVariable { id } => (vec![pair(var(id), exists)], vec![pair(var(id), whole)]),
        Command::SetVariableValue { id, mode, .. } => field(var(id), vec![mode.clone()]),
        _ => (vec![], vec![]),
    }
}
