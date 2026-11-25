//! Text that comes from a model, a server or a file, made safe to display and capped (rule U9 of
//! `docs/standards/modular-thin-ui.md`): the one set of hidden characters, two profiles, the caps, and one error type.
//! Portable (it builds for `wasm32-unknown-unknown`), with no dependency, so every layer can use it without a cycle.
//!
//! # The set
//!
//! [`is_hidden_text`] is the one answer to "does this character change how a text reads without showing, or is it not
//! text at all". It is the widest of the sets the repository used to keep in six places: the controls, the format
//! characters (category Cf) and the other characters that draw nothing or reorder a line (bidirectional controls,
//! zero-width marks, the line and paragraph separators, the soft hyphen, the braille blank and the Hangul and Khmer
//! fillers, the variation selectors, the tag characters). The list is in the doc of the function and in
//! `tests/charset.rs`, which compares every scalar value with it.
//!
//! # The two profiles (the choice, written once)
//!
//! | | [`Profile::Strict`] | [`Profile::Display`] |
//! |---|---|---|
//! | for | identifiers, actors, keys, file names, notes | text copied from a node name or a message, to be read by a person |
//! | removes | everything in [`is_hidden_text`] | everything in [`is_hidden_text`] except the two kept below |
//! | zero-width joiner U+200D | removed | kept between two pictographs (a family, a couple) |
//! | variation selector 16 U+FE0F | removed | kept after a pictograph (a heart), and after a digit, `#` or `*` that a keycap U+20E3 follows |
//! | bidirectional controls, tag characters, other selectors, every other format character | removed | removed |
//!
//! The strict profile cannot keep U+200D and U+FE0F, because a joiner and a selector are exactly what hides a second
//! meaning in a name that looks the same; an identifier has no emoji to protect. A node name does: a name such as a
//! family emoji or a heart is a joiner and a selector between emoji, and the strict set would break it into its parts.
//! So the display profile keeps those two where they make an emoji and nowhere else, and removes every other
//! invisible character, bidirectional controls (U+061C, U+200E and 200F, U+202A to 202E, U+2066 to 2069) and tag
//! characters (U+E0000 to E007F) included. A regional-indicator flag (two letters) has no hidden character and stays
//! whole. The flag of a region (England, Scotland, Wales) is a black flag followed by tag characters, so it loses its
//! tags and shows as a black flag: that is the price of removing the tag block, which is the one that carries a hidden
//! message.
//!
//! A line break is a control, so both profiles remove it. A rule that allows one (a note, a critique text) checks with
//! `keep_breaks` ([`first_hidden`]) instead of cleaning.
//!
//! # Refuse or clean
//!
//! A server that receives text it did not write refuses a bad one and writes nothing ([`plain`], [`key`],
//! [`file_name`] and the others return a [`TextError`]): a silent edit would hide what the sender tried to say. A
//! layer that must show whatever arrived cleans it ([`sanitize`], then [`clip`]), once, where the text enters the app
//! layer.
//!
//! # Caps
//!
//! A cap is checked in characters and in bytes together ([`plain`] takes both), because the two counts differ for
//! text that is not ASCII, and a rule counted one way in one place and the other way in another place accepts a text
//! that the second place refuses.

mod charset;
mod plain;
mod sanitize;

pub use charset::{first_hidden, is_combining_mark, is_hidden_text, mark_run_exceeds};
pub use plain::{TextError, actor, file_name, key, plain, workspace_name};
pub use sanitize::{Profile, clip, sanitize};

/// The longest actor string, in bytes (and so in characters): the column of a batch header and of a checkpoint holds
/// no more.
pub const MAX_ACTOR: usize = 128;
/// The longest workspace or file name, in bytes (and so in characters). With the length prefix of a room key it keeps
/// a key far under the Postgres index row limit.
pub const MAX_FILE_NAME: usize = 128;
/// The longest idempotency key, in characters.
pub const MAX_KEY: usize = 100;
/// The longest idempotency key, in bytes: [`MAX_KEY`] characters of four bytes each.
pub const MAX_KEY_BYTES: usize = 4 * MAX_KEY;
/// The longest reference to a node a tool takes, in bytes (and so in characters).
pub const MAX_NODE_REF: usize = 128;
/// The longest run of combining marks in a text: more would let stacked marks smear a line.
pub const MAX_COMBINING_RUN: usize = 4;
