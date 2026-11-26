//! The one set of characters that change how a text reads without showing, and the combining marks.

/// Whether a character is not text, or changes how a text reads without showing. This is the strict set, the one
/// every identifier, actor, key and file name is held to (see the crate docs for the display profile, which keeps two
/// of these between emoji).
///
/// Named in full: the controls (`char::is_control`, the line break among them: a rule that allows `\n` says so
/// itself), U+00AD, U+034F, U+0600 to 0605, U+061C, U+06DD, U+070F, U+0890 and 0891, U+08E2, U+115F and U+1160,
/// U+17B4 and U+17B5, U+180B to 180F, U+200B to 200F, U+2028 to 202E, U+2060 to 206F, U+2800, U+3164, U+FE00 to FE0F,
/// U+FEFF, U+FFA0, U+FFF9 to FFFB, U+110BD, U+110CD, U+13430 to 1343F, U+1BCA0 to 1BCA3, U+1D173 to 1D17A, U+E0000
/// to E007F and U+E0100 to E01EF. It is the union of what the older predicates of the activity panel (the widest of
/// them) and of the variables import (the format characters, category Cf) held.
pub fn is_hidden_text(c: char) -> bool {
    c.is_control()
        || matches!(c as u32,
            0x00AD | 0x034F | 0x0600..=0x0605 | 0x061C | 0x06DD | 0x070F | 0x0890..=0x0891 | 0x08E2
            | 0x115F..=0x1160 | 0x17B4..=0x17B5 | 0x180B..=0x180F | 0x200B..=0x200F | 0x2028..=0x202E
            | 0x2060..=0x206F | 0x2800 | 0x3164 | 0xFE00..=0xFE0F | 0xFEFF | 0xFFA0 | 0xFFF9..=0xFFFB
            | 0x110BD | 0x110CD | 0x13430..=0x1343F | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A
            | 0xE0000..=0xE007F | 0xE0100..=0xE01EF)
}

/// Whether a character is a combining mark of the blocks a run is capped in: U+0300 to 036F, 1AB0 to 1AFF, 1DC0 to
/// 1DFF, 20D0 to 20FF and FE20 to FE2F.
pub fn is_combining_mark(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

/// Whether `s` holds more than `max` combining marks in a row (see [`is_combining_mark`]). A run is counted on the
/// text as given: strip hidden characters first when they should not break a run.
pub fn mark_run_exceeds(s: &str, max: usize) -> bool {
    let mut run = 0;
    for c in s.chars() {
        run = if is_combining_mark(c) { run + 1 } else { 0 };
        if run > max {
            return true;
        }
    }
    false
}

/// The first hidden character of `s` ([`is_hidden_text`]); with `keep_breaks` a line break (`\n`) is not one.
pub fn first_hidden(s: &str, keep_breaks: bool) -> Option<char> {
    s.chars()
        .find(|c| !(keep_breaks && *c == '\n') && is_hidden_text(*c))
}

/// Whether a character is a pictograph: the blocks that hold emoji (the old symbols and arrows, the dingbats, and
/// everything from U+1F000 to 1FAFF, which has the skin tones and the regional indicators of a flag). It is a block
/// test, not the emoji table: a character in a block that is not an emoji counts, which only matters for a joiner or
/// a selector right next to it.
pub(crate) fn is_pictograph(c: char) -> bool {
    matches!(c as u32,
        0x00A9 | 0x00AE | 0x203C | 0x2049 | 0x2122 | 0x2139 | 0x2190..=0x21FF | 0x231A..=0x23FF | 0x24C2
        | 0x25AA..=0x25FE | 0x2600..=0x27BF | 0x2934..=0x2935 | 0x2B05..=0x2B55 | 0x3030 | 0x303D
        | 0x3297 | 0x3299 | 0x1F000..=0x1FAFF)
}
