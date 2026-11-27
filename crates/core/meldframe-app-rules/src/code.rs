//! A refusal code that came from outside the app layer (the server, the shell) made safe to carry in an event (rule U9): an
//! identifier cut to [`MAX_CODE`] bytes, or [`UNKNOWN_CODE`] when nothing is left. `meldframe-app`'s `text` uses it for the
//! codes of the server's answers, and the PNG export for what the shell reports.

use meldframe_text::identifier;

/// The longest refusal code kept, in bytes (and so in characters): the codes of the server are words of a few
/// characters (`quota_exceeded`, `E_INVALID_ID`).
pub const MAX_CODE: usize = 64;

/// The code a refusal that cleans to nothing is kept as, so an event never carries an empty code.
pub const UNKNOWN_CODE: &str = "unknown";

/// A refusal code from the server: an identifier cut to [`MAX_CODE`] bytes, or [`UNKNOWN_CODE`] when nothing is left.
pub fn code(raw: &str) -> String {
    let clean = identifier(raw, MAX_CODE, MAX_CODE);
    if clean.is_empty() {
        UNKNOWN_CODE.to_owned()
    } else {
        clean
    }
}
