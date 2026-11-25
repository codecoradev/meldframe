//! `var(--x[, fallback])` read from the text, as ADR-0020 does for the variables import: the custom property is
//! replaced by its value (itself read the same way), the fallback is used when there is none, and the binding to a
//! Variable of the file is not made (the literal is written, `docs/import-html.md` section 3.3). A chain of more
//! than [`MAX_DEPTH`] steps and a cycle make the whole value unresolved, and so does an expansion that grows past
//! its byte limit or takes more than [`MAX_EXPANSIONS`] replacements, so no text can make this exponential.
//! Strings are copied as they are.

use std::sync::Arc;

/// The most nested replacements (`--a: var(--b)`, `--b: var(--c)`, ...) and fallbacks inside fallbacks.
pub const MAX_DEPTH: usize = 16;
/// The most `var()` replaced in one value.
pub const MAX_EXPANSIONS: usize = 256;

/// Why a value could not be resolved. The property is left out and the element's note says which.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarError {
    /// A custom property with no value and no fallback.
    Undefined(String),
    /// A cycle, or a chain of more than [`MAX_DEPTH`] steps.
    TooDeep,
    /// The result is over the byte limit, or [`MAX_EXPANSIONS`] was passed.
    TooLong,
    /// `var(` that is not closed, or whose name is not a custom property.
    Syntax,
}

impl VarError {
    /// The stable code of the note.
    pub fn code(&self) -> &'static str {
        match self {
            VarError::Undefined(_) => "var_undefined",
            VarError::TooDeep => "var_too_deep",
            VarError::TooLong => "var_too_long",
            VarError::Syntax => "var_invalid",
        }
    }
}

struct Ctx<'a> {
    lookup: &'a dyn Fn(&str) -> Option<Arc<str>>,
    max_len: usize,
    expansions: usize,
}

/// The index of the `)` that closes the `(` before `from`, outside strings.
fn close(s: &str, from: usize) -> Option<usize> {
    let (mut depth, mut quote, b) = (1usize, None::<u8>, s.as_bytes());
    let mut i = from;
    while i < b.len() {
        match (quote, b[i]) {
            (Some(_), b'\\') | (None, b'\\') => i += 1,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, c @ (b'"' | b'\'')) => quote = Some(c),
            (None, b'(') => depth += 1,
            (None, b')') => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// The index of the first `,` at depth 0 in `s`, outside strings.
fn comma(s: &str) -> Option<usize> {
    let (mut depth, mut quote, b) = (0usize, None::<u8>, s.as_bytes());
    let mut i = 0;
    while i < b.len() {
        match (quote, b[i]) {
            (_, b'\\') => i += 1,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, c @ (b'"' | b'\'')) => quote = Some(c),
            (None, b'(') => depth += 1,
            (None, b')') => depth = depth.saturating_sub(1),
            (None, b',') if depth == 0 => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

fn sub(cx: &mut Ctx, s: &str, depth: usize, out: &mut String) -> Result<(), VarError> {
    if depth > MAX_DEPTH {
        return Err(VarError::TooDeep);
    }
    let b = s.as_bytes();
    let (mut i, mut copied, mut quote) = (0, 0, None::<u8>);
    while i < b.len() {
        match (quote, b[i]) {
            (Some(_), b'\\') | (None, b'\\') => i += 1,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, c @ (b'"' | b'\'')) => quote = Some(c),
            (None, b'v' | b'V')
                if b[i..].len() >= 4
                    && b[i..i + 4].eq_ignore_ascii_case(b"var(")
                    && !(i > 0
                        && (b[i - 1].is_ascii_alphanumeric()
                            || b[i - 1] == b'-'
                            || b[i - 1] == b'_')) =>
            {
                let end = close(s, i + 4).ok_or(VarError::Syntax)?;
                out.push_str(&s[copied..i]);
                let inner = &s[i + 4..end];
                let (name, fallback) = match comma(inner) {
                    Some(k) => (&inner[..k], Some(&inner[k + 1..])),
                    None => (inner, None),
                };
                let name = name.trim();
                if !name.starts_with("--") || name.len() < 3 {
                    return Err(VarError::Syntax);
                }
                cx.expansions += 1;
                if cx.expansions > MAX_EXPANSIONS {
                    return Err(VarError::TooLong);
                }
                match ((cx.lookup)(name), fallback) {
                    (Some(v), _) => sub(cx, &v, depth + 1, out)?,
                    (None, Some(f)) => sub(cx, f.trim(), depth + 1, out)?,
                    (None, None) => return Err(VarError::Undefined(name.to_owned())),
                }
                if out.len() > cx.max_len {
                    return Err(VarError::TooLong);
                }
                copied = end + 1;
                i = end;
            }
            _ => {}
        }
        i += 1;
    }
    out.push_str(&s[copied.min(s.len())..]);
    if out.len() > cx.max_len {
        return Err(VarError::TooLong);
    }
    Ok(())
}

/// Replace every `var()` in `value`. `lookup` gives the raw value of a custom property (which may hold `var()`
/// itself). The result is at most `max_len` bytes. A value with no `var(` comes back as it is.
pub fn resolve(
    value: &str,
    lookup: &dyn Fn(&str) -> Option<Arc<str>>,
    max_len: usize,
) -> Result<String, VarError> {
    let mut cx = Ctx {
        lookup,
        max_len,
        expansions: 0,
    };
    let mut out = String::new();
    sub(&mut cx, value, 0, &mut out)?;
    Ok(out)
}
