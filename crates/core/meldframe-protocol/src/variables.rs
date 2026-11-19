//! Variables: collections with modes, typed variables, aliases and the binding of a node field (ADR-0004,
//! ADR-0017). Everything here is data; the rules live in `meldframe-variables` (resolve, alias chains) and
//! `meldframe-check` (validation).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// A reference to a variable by its stable id (never its name, so a rename touches no binding).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct VarRef {
    pub var: String,
}

/// A field value that is either a literal `T` or a binding `{"var": id}` (E3.3). A file written before
/// bindings existed holds literals only, so it stays valid.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum Bind<T> {
    Lit(T),
    Var(VarRef),
}

impl<T> Bind<T> {
    /// A binding to variable `id`.
    pub fn var(id: impl Into<String>) -> Self {
        Bind::Var(VarRef { var: id.into() })
    }

    /// The literal, if this is not a binding.
    pub fn lit(&self) -> Option<&T> {
        match self {
            Bind::Lit(v) => Some(v),
            Bind::Var(_) => None,
        }
    }

    /// The id of the bound variable, if this is a binding.
    pub fn var_id(&self) -> Option<&str> {
        match self {
            Bind::Var(r) => Some(&r.var),
            Bind::Lit(_) => None,
        }
    }
}

impl From<&str> for Bind<String> {
    fn from(s: &str) -> Self {
        Bind::Lit(s.to_string())
    }
}

impl From<String> for Bind<String> {
    fn from(s: String) -> Self {
        Bind::Lit(s)
    }
}

/// A literal equals the value it holds; a binding equals no literal.
impl PartialEq<&str> for Bind<String> {
    fn eq(&self, other: &&str) -> bool {
        self.lit().is_some_and(|s| s == other)
    }
}

impl PartialEq<f64> for Bind<f64> {
    fn eq(&self, other: &f64) -> bool {
        self.lit() == Some(other)
    }
}

impl From<f64> for Bind<f64> {
    fn from(v: f64) -> Self {
        Bind::Lit(v)
    }
}

/// What a variable holds. Strings, shadows, gradients and typography come later (ADR-0004); a new type is an
/// additive value of this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum VarType {
    /// A hex colour (`#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`).
    Color,
    /// A finite number, unitless (pixels on the canvas).
    Number,
}

/// A literal value of a variable: a hex colour or a number.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum Value {
    Color(String),
    Number(f64),
}

impl Value {
    /// The type of this literal.
    pub fn var_type(&self) -> VarType {
        match self {
            Value::Color(_) => VarType::Color,
            Value::Number(_) => VarType::Number,
        }
    }
}

/// One alternative set of values of a collection (light, dark).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct Mode {
    pub id: String,
    pub name: String,
}

/// A group of variables that share modes. The first mode is the default one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub modes: Vec<Mode>,
}

/// A named, typed value per mode. `values` has exactly one entry for every mode of the collection; an entry is
/// a literal of the variable's type or an alias (`{"var": id}`) to another variable of the same type.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct Variable {
    pub id: String,
    pub collection: String,
    /// A slash path of lowercase segments, such as `color/brand/500`; unique within the collection.
    pub name: String,
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub kind: VarType,
    /// Mode id to value.
    pub values: BTreeMap<String, Bind<Value>>,
}

/// All collections and variables of a file, in the order they were added.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct Variables {
    #[serde(default)]
    pub collections: Vec<Collection>,
    #[serde(default)]
    pub variables: Vec<Variable>,
}

impl Variables {
    /// True when the file holds no collection and no variable.
    pub fn is_empty(&self) -> bool {
        self.collections.is_empty() && self.variables.is_empty()
    }

    /// The collection with this id.
    pub fn collection(&self, id: &str) -> Option<&Collection> {
        self.collections.iter().find(|c| c.id == id)
    }

    /// The variable with this id.
    pub fn variable(&self, id: &str) -> Option<&Variable> {
        self.variables.iter().find(|v| v.id == id)
    }
}

/// The content of a file: its nodes and its variables. Commands apply to a `Doc`.
#[derive(Debug, Clone, Default)]
pub struct Doc {
    pub nodes: Vec<crate::Node>,
    pub vars: Variables,
}

impl Doc {
    /// A document of nodes only.
    pub fn of_nodes(nodes: Vec<crate::Node>) -> Self {
        Doc {
            nodes,
            vars: Variables::default(),
        }
    }
}
