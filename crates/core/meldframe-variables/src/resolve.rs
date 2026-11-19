use meldframe_protocol::{Bind, Value, Variable, Variables};
use std::collections::BTreeMap;

/// The longest chain of aliases `resolve` follows (and the longest a command may build).
pub const MAX_ALIAS_DEPTH: usize = 16;

/// Which mode a viewer looks at, per collection id. A collection that is not in the choice, or whose chosen
/// mode does not exist, shows its default (first) mode. It is view state: it is never written to the file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModeChoice(BTreeMap<String, String>);

impl ModeChoice {
    /// A choice from `(collection id, mode id)` pairs.
    pub fn of<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        ModeChoice(
            pairs
                .into_iter()
                .map(|(c, m)| (c.to_string(), m.to_string()))
                .collect(),
        )
    }

    /// Pick `mode` for `collection`.
    pub fn set(&mut self, collection: &str, mode: &str) {
        self.0.insert(collection.to_string(), mode.to_string());
    }

    /// Show the default mode of `collection` again.
    pub fn clear(&mut self, collection: &str) {
        self.0.remove(collection);
    }

    /// The mode id picked for `collection`, if any.
    pub fn get(&self, collection: &str) -> Option<&str> {
        self.0.get(collection).map(String::as_str)
    }

    /// Every pick, by collection id.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(c, m)| (c.as_str(), m.as_str()))
    }
}

/// The value of variable `id` for the modes in `choice`. An alias is followed with the choice for the
/// collection of the variable it points at, so one switch of `theme` reaches the variables that `theme` aliases
/// in other collections. `None` when the variable is missing, the chain has a cycle or is longer than
/// [`MAX_ALIAS_DEPTH`], or a value does not have the variable's type; it never panics.
pub fn resolve(vars: &Variables, id: &str, choice: &ModeChoice) -> Option<Value> {
    let mut cur = vars.variable(id)?;
    for _ in 0..=MAX_ALIAS_DEPTH {
        match value_in(vars, cur, choice)? {
            Bind::Lit(v) => return (v.var_type() == cur.kind).then(|| v.clone()),
            Bind::Var(r) => cur = vars.variable(&r.var)?,
        }
    }
    None
}

/// The value of `v` in the mode `choice` picks for its collection, else in the default mode.
fn value_in<'a>(
    vars: &'a Variables,
    v: &'a Variable,
    choice: &ModeChoice,
) -> Option<&'a Bind<Value>> {
    let col = vars.collection(&v.collection)?;
    let picked = choice
        .get(&col.id)
        .filter(|m| col.modes.iter().any(|x| x.id == *m))
        .or_else(|| col.modes.first().map(|m| m.id.as_str()))?;
    v.values.get(picked).or_else(|| {
        let default = col.modes.first()?;
        v.values.get(&default.id)
    })
}
