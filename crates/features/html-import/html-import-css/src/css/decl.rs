//! One declaration, `name: value [!important]`, from the text of a piece. Property names are folded to ASCII lower
//! case, except a custom property (`--x`), whose name is case-sensitive. The value is kept as written (comments are
//! already gone), trimmed. Nothing is evaluated here.

use std::sync::Arc;

/// A declaration of a rule or of a `style` attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decl {
    pub name: Arc<str>,
    pub value: Arc<str>,
    pub important: bool,
}

/// Why a piece is not a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bad {
    /// No colon, no name, or a name that is not one.
    Syntax,
    /// No value (an empty value is only valid for a custom property).
    Empty,
    /// The value is over the limit.
    TooLong,
}

fn property_name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        && !s.starts_with(|c: char| c.is_ascii_digit())
}

/// `value` without a trailing `!important` (any case, any space around the `!`), and whether it had one.
fn important(value: &str) -> (&str, bool) {
    let v = value.trim_ascii_end();
    let b = v.as_bytes();
    if b.len() >= 9 && b[b.len() - 9..].eq_ignore_ascii_case(b"important") {
        let head = v[..v.len() - 9].trim_ascii_end();
        if let Some(h) = head.strip_suffix('!') {
            return (h.trim_ascii_end(), true);
        }
    }
    (v, false)
}

/// Read the declaration in `piece`. `too_long` says the piece was cut by the reader's cap. `max_value` is the most
/// bytes of a value.
pub fn parse(piece: &str, too_long: bool, max_value: usize) -> Result<Decl, Bad> {
    let (name, value) = piece.split_once(':').ok_or(Bad::Syntax)?;
    let name = name.trim_ascii();
    if !property_name(name) {
        return Err(Bad::Syntax);
    }
    if too_long {
        return Err(Bad::TooLong);
    }
    let (value, important) = important(value.trim_ascii());
    if value.len() > max_value {
        return Err(Bad::TooLong);
    }
    let custom = name.starts_with("--");
    if value.is_empty() && !custom {
        return Err(Bad::Empty);
    }
    Ok(Decl {
        name: if custom {
            name.into()
        } else {
            name.to_ascii_lowercase().into()
        },
        value: value.into(),
        important,
    })
}
