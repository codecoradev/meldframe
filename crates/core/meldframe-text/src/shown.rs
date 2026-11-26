//! Text made safe to show or to keep as a key, as a value: the two calls the app layer makes where text from a model, a
//! server or a file enters it (rule U9). Both build on [`sanitize_clip`]; the choice of profile is written in the crate
//! docs.

use crate::MAX_COMBINING_RUN;
use crate::charset::is_combining_mark;
use crate::sanitize::{Profile, sanitize, sanitize_clip};

/// A name or a label made safe for a person to read: the display profile ([`sanitize`], so a joiner and a selector
/// stay between emoji), no more than [`MAX_COMBINING_RUN`] combining marks in a row (counted once the hidden characters
/// between them are gone), every run of white space of any kind (a no-break space, an ideographic space) one plain
/// space so a name cannot pass for the badge beside it (a tab or a line break is one too, not a removed character), the
/// ends trimmed, and the text cut to `max_chars` characters
/// ([`crate::MAX_LABEL`] is the longest a name of the file is). The cut text is made safe once more, so no joiner is left that
/// joins nothing, and trimmed again. A text that is only hidden or white space comes back empty: the caller decides
/// what an empty name shows (the stored text is not changed by this call).
pub fn label(s: &str, max_chars: usize) -> String {
    // A tab or a line break is white space that a name may hold and that reads as a gap: it becomes a space rather than
    // joining the two words around it (every other control character is removed).
    let spaced = s.replace(['\t', '\n', '\r', '\u{0B}', '\u{0C}'], " ");
    let safe = sanitize(&spaced, Profile::Display);
    let mut out = String::with_capacity(safe.len());
    let (mut run, mut gap) = (0usize, false);
    for c in safe.chars() {
        run = if is_combining_mark(c) { run + 1 } else { 0 };
        if run > MAX_COMBINING_RUN {
            continue;
        }
        if c.is_whitespace() {
            gap = true;
            continue;
        }
        if gap && !out.is_empty() {
            out.push(' ');
        }
        gap = false;
        out.push(c);
    }
    let cut = sanitize_clip(&out, Profile::Display, max_chars, usize::MAX);
    cut.trim_end().to_owned()
}

/// An identifier (an actor, a key, a refusal code) made safe: the strict profile ([`sanitize`], no exception) and
/// the cut to `max_chars` characters and `max_bytes` bytes, between characters. Nothing else changes, white space
/// included, so that an identifier a server accepted comes back as it was: the rule of [`crate::actor`] and
/// [`crate::key`] is a fixed point of this call for every text it takes. An identifier is safe to show as it is.
pub fn identifier(s: &str, max_chars: usize, max_bytes: usize) -> String {
    sanitize_clip(s, Profile::Strict, max_chars, max_bytes)
}
