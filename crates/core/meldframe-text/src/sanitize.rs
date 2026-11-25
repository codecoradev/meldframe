//! Making a text safe to show: `sanitize` by profile, and `clip` to a length.

use crate::charset::{is_hidden_text, is_pictograph};
use std::borrow::Cow;

const ZWJ: char = '\u{200D}';
const VS16: char = '\u{FE0F}';
const KEYCAP: char = '\u{20E3}';

/// What a text is for, which decides what [`sanitize`] keeps. The choice is written once, in the crate docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Identifiers, actors, keys and file names: every hidden character goes, with no exception.
    Strict,
    /// Text copied from a node name or a message for a person to read: as `Strict`, except that a zero-width joiner
    /// and the variation selector 16 stay where they make an emoji (a family, a heart with its selector, a keycap).
    Display,
}

/// `s` with the hidden characters of `profile` removed (the set of [`is_hidden_text`]; the controls, the line break
/// among them, go in both profiles). A text that has none comes back as it is, not copied. Nothing is cut: see
/// [`clip`] for the length.
pub fn sanitize(s: &str, profile: Profile) -> Cow<'_, str> {
    let mut out = String::with_capacity(s.len());
    let mut rest = s.chars().peekable();
    while let Some(c) = rest.next() {
        let keep = if !is_hidden_text(c) {
            true
        } else if profile == Profile::Display {
            let prev = out.chars().next_back();
            match c {
                ZWJ => {
                    prev.is_some_and(|p| is_pictograph(p) || p == VS16)
                        && rest.peek().is_some_and(|n| is_pictograph(*n))
                }
                VS16 => {
                    prev.is_some_and(is_pictograph)
                        || (prev.is_some_and(|p| matches!(p, '0'..='9' | '#' | '*'))
                            && rest.peek() == Some(&KEYCAP))
                }
                _ => false,
            }
        } else {
            false
        };
        if keep {
            out.push(c);
        }
    }
    // Only removals happen, so the same length is the same text.
    if out.len() == s.len() {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(out)
    }
}

/// The longest start of `s` with at most `max_chars` characters (Unicode scalar values) and at most `max_bytes`
/// bytes, cut between characters. Pass `usize::MAX` for a bound that does not apply.
pub fn clip(s: &str, max_chars: usize, max_bytes: usize) -> &str {
    let mut end = 0;
    for (n, (i, c)) in s.char_indices().enumerate() {
        let next = i + c.len_utf8();
        if n >= max_chars || next > max_bytes {
            return &s[..end];
        }
        end = next;
    }
    s
}
