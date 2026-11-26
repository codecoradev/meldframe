//! The shapes of the ids the model names things with, written once. A caller that needs the answer as an error (the
//! checks of a file, the asset store, the export, the importer) calls these and maps `false` to its own.

/// Whether `id` is an id of a node, a collection, a mode or a variable: 1 to `max` ASCII letters, digits or
/// `_`, `-`, `:` and `.`. The cap is the caller's (a file's limit, or the 64 a tool states in its schema). Only ASCII
/// passes, so a character is a byte and the cap is a byte length, tested first: an id of any size is refused before
/// its characters are read. The charset is the rule, so a path, a selector or a log line built from an id that passes
/// needs no escaping.
pub fn valid_id(id: &str, max: usize) -> bool {
    !id.is_empty()
        && id.len() <= max
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '.'))
}

/// The length of an asset id, in characters (and bytes): the lower-case hex of a SHA-256.
pub const ASSET_ID_LEN: usize = 64;

/// Whether `s` is not empty and made only of the digits `0` to `9` and the lower-case letters `a` to `f`.
pub fn is_lower_hex(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Whether `id` has the form of an asset id (`image.assetId`): [`ASSET_ID_LEN`] lower-case hex digits, the SHA-256 of
/// the bytes. It says nothing about whether such an asset exists, and a path built from an id that passes is safe.
pub fn is_asset_id(id: &str) -> bool {
    id.len() == ASSET_ID_LEN && is_lower_hex(id)
}
