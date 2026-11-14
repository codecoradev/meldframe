//! Why a command is refused.

use std::fmt;

/// The reason a command was refused. A refused command leaves the file unchanged.
#[derive(Debug, Clone, PartialEq)]
pub enum ApplyError {
    /// A node id does not exist in the file.
    NotFound(String),
    /// A node with this id already exists.
    DuplicateId(String),
    /// The new parent does not exist.
    ParentNotFound(String),
    /// The new parent is not a frame; only frames hold children.
    ParentNotFrame(String),
    /// A reparent would make a node its own ancestor.
    Cycle,
    /// The file would hold more nodes than `Limits::max_nodes`.
    TooManyNodes(usize),
    /// The tree would nest deeper than `Limits::max_depth`.
    TooDeep(usize),
    /// A node id is empty, too long or uses characters other than letters, digits and `_-:.`.
    InvalidId(String),
    /// A number is NaN or infinite.
    NotFinite(&'static str),
    /// A number is outside the range its field allows.
    OutOfRange { field: &'static str, value: f64 },
    /// A string is longer than the limit.
    TooLong { field: &'static str, max: usize },
    /// A colour is not hex (`#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`). A free-form value would stop the
    /// front end from rendering.
    InvalidColor { field: &'static str },
    /// A list field has more entries than allowed (`shadows`).
    TooMany { field: &'static str, max: usize },
    /// `fontWeight` is not 100..=900 in steps of 100.
    InvalidFontWeight(u16),
    /// A `pos` from a client has bad characters or a bad shape.
    InvalidPos(String),
    /// A `pos` (sent or computed) is longer than `Limits::max_pos`: the gap is too crowded.
    PosTooLong { max: usize },
    /// A `Reorder` is not coherent: more than one of `before`/`after`/`pos`, or the anchor is not a sibling.
    BadReorder(String),
}

impl ApplyError {
    /// The stable machine code of this error (`E_PARENT_NOT_FOUND`, ...). It never changes once released
    /// and does not depend on the details; `docs/error-codes.md` lists every code. The match has no
    /// wildcard, so a new variant cannot be added without a code.
    pub fn code(&self) -> &'static str {
        match self {
            ApplyError::NotFound(_) => "E_NODE_NOT_FOUND",
            ApplyError::DuplicateId(_) => "E_DUPLICATE_ID",
            ApplyError::ParentNotFound(_) => "E_PARENT_NOT_FOUND",
            ApplyError::ParentNotFrame(_) => "E_PARENT_NOT_FRAME",
            ApplyError::Cycle => "E_CYCLE",
            ApplyError::TooManyNodes(_) => "E_TOO_MANY_NODES",
            ApplyError::TooDeep(_) => "E_TOO_DEEP",
            ApplyError::InvalidId(_) => "E_INVALID_ID",
            ApplyError::NotFinite(_) => "E_NOT_FINITE",
            ApplyError::OutOfRange { .. } => "E_OUT_OF_RANGE",
            ApplyError::TooLong { .. } => "E_TOO_LONG",
            ApplyError::InvalidColor { .. } => "E_INVALID_COLOR",
            ApplyError::InvalidPos(_) => "E_INVALID_POS",
            ApplyError::PosTooLong { .. } => "E_POS_TOO_LONG",
            ApplyError::BadReorder(_) => "E_BAD_REORDER",
            ApplyError::TooMany { .. } => "E_TOO_MANY_ENTRIES",
            ApplyError::InvalidFontWeight(_) => "E_INVALID_FONT_WEIGHT",
        }
    }
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApplyError::NotFound(id) => write!(f, "node not found: {id}"),
            ApplyError::DuplicateId(id) => write!(f, "id already in use: {id}"),
            ApplyError::ParentNotFound(id) => write!(f, "parent not found: {id}"),
            ApplyError::ParentNotFrame(id) => write!(f, "parent is not a frame: {id}"),
            ApplyError::Cycle => write!(f, "reparent would create a cycle"),
            ApplyError::TooManyNodes(n) => write!(f, "node limit exceeded ({n})"),
            ApplyError::TooDeep(n) => write!(f, "maximum nesting depth exceeded ({n})"),
            ApplyError::InvalidId(id) => write!(f, "invalid id: {id:?}"),
            ApplyError::NotFinite(field) => write!(f, "{field} is not a finite number"),
            ApplyError::OutOfRange { field, value } => {
                write!(f, "{field} is out of range: {value}")
            }
            ApplyError::TooLong { field, max } => write!(f, "{field} exceeds {max} characters"),
            ApplyError::InvalidColor { field } => {
                write!(
                    f,
                    "{field} must be a hex colour (#rgb, #rgba, #rrggbb or #rrggbbaa)"
                )
            }
            ApplyError::TooMany { field, max } => {
                write!(f, "{field} has too many entries (max {max})")
            }
            ApplyError::InvalidFontWeight(w) => {
                write!(f, "fontWeight must be 100..=900 in steps of 100, got {w}")
            }
            ApplyError::InvalidPos(p) => write!(f, "invalid pos: {p:?}"),
            ApplyError::BadReorder(m) => write!(f, "invalid reorder: {m}"),
            ApplyError::PosTooLong { max } => {
                write!(
                    f,
                    "pos exceeds {max} characters: the sibling gap is too crowded (rebalance)"
                )
            }
        }
    }
}

impl std::error::Error for ApplyError {}
