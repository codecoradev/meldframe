//! Where text the server sends enters the app layer (rule U9): an actor, an idempotency key and a refusal code are
//! identifiers, so each is made safe with the strict profile of `meldframe-text` and cut, once, here, and everything
//! after this point (lane keys, rows, the layers' last actor, events, state) holds the clean text.
//!
//! Not changed by this: what the replica and the file hold. An op is applied as the server sent it, under the actor
//! and the key it sent, so the file never drifts from the server's; the clean text is for what the app shows and keys by.
//! A text the server could have sent (it holds every actor and key to `meldframe_text::actor` and `key`) comes back
//! unchanged, so this only acts on a server that broke its own rule.
//!
//! The profile of each text of the state is written once, in `docs/architecture.md` section 3c.

use meldframe_text::{MAX_ACTOR, MAX_KEY, MAX_KEY_BYTES, identifier};

/// The longest refusal code kept, in bytes (and so in characters): the codes of the server are words of a few
/// characters (`quota_exceeded`, `E_INVALID_ID`).
pub(crate) const MAX_CODE: usize = 64;

/// The code a refusal that cleans to nothing is kept as, so an event never carries an empty code.
pub(crate) const UNKNOWN_CODE: &str = "unknown";

/// The key of the lane, the layer's last actor and the actor of a change event: the actor of a frame as an identifier.
/// `None` when nothing is left of it; the op is still applied to the file, and no lane, row or attribution is made
/// for an actor with no name.
pub(crate) fn actor(raw: &str) -> Option<String> {
    let clean = identifier(raw, MAX_ACTOR, MAX_ACTOR);
    (!clean.is_empty()).then_some(clean)
}

/// The idempotency key of a confirmed op as the log and the reducer see it (a batch is `{prefix}:{n}`).
pub(crate) fn op_key(raw: &str) -> String {
    identifier(raw, MAX_KEY, MAX_KEY_BYTES)
}

/// A refusal code from the server: an identifier cut to [`MAX_CODE`] bytes, or [`UNKNOWN_CODE`] when nothing is left.
pub(crate) fn code(raw: &str) -> String {
    let clean = identifier(raw, MAX_CODE, MAX_CODE);
    if clean.is_empty() {
        UNKNOWN_CODE.to_owned()
    } else {
        clean
    }
}
