//! The upload token: a secret that must not leak into logs.

use crate::CaptureError;

const MIN_LEN: usize = 16;

/// An upload token of at least 16 bytes. Its `Debug` output is redacted.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// Wrap a token; the caller makes it unguessable (random, 16 bytes or more).
    pub fn new(s: &str) -> Result<Token, CaptureError> {
        if s.len() < MIN_LEN {
            return Err(CaptureError::Invalid(format!(
                "the upload token must be at least {MIN_LEN} bytes"
            )));
        }
        Ok(Token(s.to_string()))
    }

    /// The token text, for the request frame.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn matches(&self, presented: &str) -> bool {
        ct_eq(self.0.as_bytes(), presented.as_bytes())
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(<redacted>)")
    }
}

/// Equality that does not stop at the first difference (equal lengths are compared in full).
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
