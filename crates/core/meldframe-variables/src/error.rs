//! Why a variable command, a binding or a stored variable is refused.

use meldframe_protocol::VarType;
use std::fmt;

/// The codes of the errors that are specific to variables, as `docs/error-codes.md` lists them. A test in
/// `meldframe-variables-tests` keeps this list, [`VarError::code`] and the docs table the same.
pub const VAR_ERROR_CODES: &[&str] = &[
    "E_VARIABLE_NOT_FOUND",
    "E_COLLECTION_NOT_FOUND",
    "E_MODE_NOT_FOUND",
    "E_INVALID_NAME",
    "E_DUPLICATE_NAME",
    "E_TYPE_MISMATCH",
    "E_ALIAS_CYCLE",
    "E_ALIAS_TOO_DEEP",
    "E_VARIABLE_IN_USE",
    "E_COLLECTION_NOT_EMPTY",
    "E_DEFAULT_MODE",
    "E_INCOMPLETE_VALUES",
    "E_INVALID_MODES",
    "E_NO_VARIABLES",
];

/// A refusal about variables. `meldframe-check` wraps it as `ApplyError::Variable`, which keeps its code.
#[derive(Debug, Clone, PartialEq)]
pub enum VarError {
    /// A variable id does not exist (in a binding, an alias or a command).
    VariableNotFound(String),
    /// A collection id does not exist.
    CollectionNotFound(String),
    /// A mode id does not exist in the collection.
    ModeNotFound(String),
    /// A name is not a slash path of lowercase segments (`color/brand-500`).
    InvalidName(String),
    /// Another variable of the collection has this name.
    DuplicateName(String),
    /// A value, an alias or a binding has another type than the variable or field it is for.
    TypeMismatch {
        field: String,
        expected: VarType,
        got: VarType,
    },
    /// An alias would close a cycle: the variable named is reached from the variable it points at.
    AliasCycle(String),
    /// An alias chain would be longer than the limit.
    AliasTooDeep(usize),
    /// A variable that something still uses cannot be removed; `by` holds up to 20 of the nodes and variables; the message names five.
    VariableInUse { id: String, by: Vec<String> },
    /// A collection that still holds variables cannot be removed.
    CollectionNotEmpty(String),
    /// The default (first) mode cannot be removed, and no mode goes before it.
    DefaultMode(String),
    /// The values of a variable are not exactly one for each mode of its collection.
    IncompleteValues(String),
    /// A collection's modes are not valid: none, or a repeated id; the text says which.
    InvalidModes(&'static str),
    /// A variable command reached a caller that holds no variables (`check_command`, which knows nodes only).
    NoVariables,
    /// A value rule that nodes share (an id, a colour, a number, a length, a count): the stable code of that
    /// rule (`E_INVALID_ID`, `E_INVALID_COLOR`, `E_NOT_FINITE`, `E_OUT_OF_RANGE`, `E_TOO_LONG`,
    /// `E_TOO_MANY_ENTRIES`, `E_DUPLICATE_ID`) and its message.
    Common { code: &'static str, message: String },
}

impl VarError {
    /// The stable machine code (`docs/error-codes.md`).
    pub fn code(&self) -> &'static str {
        match self {
            VarError::VariableNotFound(_) => "E_VARIABLE_NOT_FOUND",
            VarError::CollectionNotFound(_) => "E_COLLECTION_NOT_FOUND",
            VarError::ModeNotFound(_) => "E_MODE_NOT_FOUND",
            VarError::InvalidName(_) => "E_INVALID_NAME",
            VarError::DuplicateName(_) => "E_DUPLICATE_NAME",
            VarError::TypeMismatch { .. } => "E_TYPE_MISMATCH",
            VarError::AliasCycle(_) => "E_ALIAS_CYCLE",
            VarError::AliasTooDeep(_) => "E_ALIAS_TOO_DEEP",
            VarError::VariableInUse { .. } => "E_VARIABLE_IN_USE",
            VarError::CollectionNotEmpty(_) => "E_COLLECTION_NOT_EMPTY",
            VarError::DefaultMode(_) => "E_DEFAULT_MODE",
            VarError::IncompleteValues(_) => "E_INCOMPLETE_VALUES",
            VarError::InvalidModes(_) => "E_INVALID_MODES",
            VarError::NoVariables => "E_NO_VARIABLES",
            VarError::Common { code, .. } => code,
        }
    }
}

impl fmt::Display for VarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VarError::VariableNotFound(id) => write!(f, "variable not found: {id}"),
            VarError::CollectionNotFound(id) => write!(f, "collection not found: {id}"),
            VarError::ModeNotFound(id) => write!(f, "mode not found: {id}"),
            VarError::InvalidName(n) => write!(
                f,
                "invalid name {n:?}: use lowercase segments joined by `/`, such as color/brand-500"
            ),
            VarError::DuplicateName(n) => {
                write!(f, "another variable of the collection is named {n:?}")
            }
            VarError::TypeMismatch {
                field,
                expected,
                got,
            } => write!(f, "{field} needs a {expected:?} value, got {got:?}"),
            VarError::AliasCycle(id) => write!(f, "an alias would make {id} depend on itself"),
            VarError::AliasTooDeep(max) => {
                write!(f, "an alias chain may have at most {max} steps")
            }
            VarError::VariableInUse { id, by } => {
                // A few names are enough to find it; the list can hold twenty ids of 128 characters.
                let shown = by.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
                let more = if by.len() > 5 { " and others" } else { "" };
                write!(f, "variable {id} is still used by {shown}{more}")
            }
            VarError::CollectionNotEmpty(id) => {
                write!(
                    f,
                    "collection {id} still holds variables: remove them first"
                )
            }
            VarError::DefaultMode(id) => write!(
                f,
                "mode {id} is or would take the place of the default (first) mode of its collection"
            ),
            VarError::IncompleteValues(id) => write!(
                f,
                "variable {id} needs exactly one value for each mode of its collection"
            ),
            VarError::InvalidModes(why) => write!(f, "invalid modes: {why}"),
            VarError::NoVariables => write!(f, "this caller holds no variables"),
            VarError::Common { message, .. } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for VarError {}
