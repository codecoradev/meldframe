//! Why a command is refused.

use std::fmt;

/// The reason a command was refused. A refused command leaves the file unchanged.
#[derive(Debug, Clone, PartialEq)]
pub enum ApplyError {
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
    /// A `pos` from a client has bad characters or a bad shape.
    InvalidPos(String),
    /// A `pos` (sent or computed) is longer than `Limits::max_pos`: the gap is too crowded.
    PosTooLong { max: usize },
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            ApplyError::InvalidPos(p) => write!(f, "invalid pos: {p:?}"),
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
