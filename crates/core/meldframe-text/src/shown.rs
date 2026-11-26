//! Text made safe to show or to keep as a key, as a value: the two calls the app layer makes where text from a model, a
//! server or a file enters it (rule U9). Both build on [`sanitize_clip`]; the choice of profile is written in the crate
//! docs.

use crate::MAX_COMBINING_RUN;
use crate::charset::is_combining_mark;
use crate::sanitize::{Profile, clip, sanitize, sanitize_clip};

/// A name or a label made safe for a person to read: the display profile ([`sanitize`], so a joiner and a selector
/// stay between emoji), no more than [`MAX_COMBINING_RUN`] combining marks in a row (counted once the hidden characters
/// between them are gone), every run of white space of any kind (a no-break space, an ideographic space) one plain
/// space so a name cannot pass for the badge beside it (a tab or a line break is one too, not a removed character), the
/// ends trimmed, and the text cut to `max_chars` characters
/// ([`crate::MAX_LABEL`] is the longest a name of the file is). The cut text is made safe once more, so no joiner is left that
/// joins nothing, and trimmed again. A text that is only hidden or white space comes back empty: the caller decides
/// what an empty name shows (the stored text is not changed by this call).
pub fn label(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars && is_plain_ascii(s) {
        return s.to_owned();
    }
    // A text far longer than anything that can be shown is cut before it is walked (see `PRE_CLIP_BYTES`).
    let s = clip(s, usize::MAX, max_chars.saturating_mul(PRE_CLIP_BYTES));
    // A tab or a line break is white space that a name may hold and that reads as a gap: it becomes a space rather than
    // joining the two words around it. So is every other line break of Unicode (next line, line and paragraph
    // separator); every other control character is removed.
    let spaced = s.replace(
        [
            '\t', '\n', '\r', '\u{0B}', '\u{0C}', '\u{85}', '\u{2028}', '\u{2029}',
        ],
        " ",
    );
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

/// How many bytes of input one shown character may take before the walk gives up on the rest: a name is read from the
/// start, and a text longer than `max_chars` times this is cut there first, so a megabyte of text costs no more than a
/// page of it. A name that hides its visible text behind more hidden characters than this loses the tail of it.
const PRE_CLIP_BYTES: usize = 64;

/// Printable ASCII with no space at either end and no two spaces in a row: a text `label` would return as it is.
fn is_plain_ascii(s: &str) -> bool {
    s.bytes().all(|b| (0x20..0x7F).contains(&b))
        && !s.starts_with(' ')
        && !s.ends_with(' ')
        && !s.contains("  ")
}

/// An identifier (an actor, a key, a refusal code) made safe: the strict profile ([`sanitize`], no exception), no more
/// than [`MAX_COMBINING_RUN`] combining marks in a row (counted once the hidden characters between them are gone, as
/// [`label`] counts), and the cut to `max_chars` characters and `max_bytes` bytes, between characters. Nothing else
/// changes, white space included, so that an identifier a server accepted comes back as it was: the rule of
/// [`crate::actor`] and [`crate::key`], which refuse a run of more than four marks, is a fixed point of this call for
/// every text it takes. An identifier is safe to show as it is.
pub fn identifier(s: &str, max_chars: usize, max_bytes: usize) -> String {
    // The most the cut can keep is `max_bytes` (and four bytes for each of `max_chars`): a text far past that is cut first.
    let keep = max_bytes.min(max_chars.saturating_mul(4));
    let s = clip(s, usize::MAX, keep.saturating_mul(PRE_CLIP_BYTES / 8));
    let safe = sanitize(s, Profile::Strict);
    let mut capped = String::with_capacity(safe.len());
    let mut run = 0usize;
    for c in safe.chars() {
        run = if is_combining_mark(c) { run + 1 } else { 0 };
        if run <= MAX_COMBINING_RUN {
            capped.push(c);
        }
    }
    clip(&capped, max_chars, max_bytes).to_owned()
}
