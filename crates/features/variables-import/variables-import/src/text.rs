//! Text for a report: what is cut and cleaned before the text of a style sheet is echoed to the reader of a report.

/// A character that is not text: a control character, any format character (Unicode category Cf: the soft hyphen, the
/// Arabic marks, the bidi controls, zero-width marks, the word joiner and the invisible operators, the byte order mark,
/// the annotation anchors, the shorthand and musical format controls), a variation selector, or a tag character (the
/// block that hides a message inside text). What reads differently from what it shows, or does not show at all, is
/// not echoed to the reader of a report.
fn invisible(c: char) -> bool {
    c.is_control()
        || matches!(c as u32,
            0xAD | 0x600..=0x605 | 0x61C | 0x6DD | 0x70F | 0x890..=0x891 | 0x8E2 | 0x180E | 0x200B..=0x200F
            | 0x202A..=0x202E | 0x2060..=0x206F | 0xFE00..=0xFE0F | 0xFEFF | 0xFFF9..=0xFFFB | 0x110BD | 0x110CD
            | 0x13430..=0x1343F | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A | 0xE0000..=0xE007F | 0xE0100..=0xE01EF)
}

/// `s` without what is not text (see `invisible`), cut to `max` characters.
pub(crate) fn clean(s: &str, max: usize) -> String {
    let mut out: String = s.chars().filter(|c| !invisible(*c)).take(max + 1).collect();
    if out.chars().count() > max {
        out = out.chars().take(max.saturating_sub(3)).collect();
        out.push_str("...");
    }
    out
}

/// One line of at most 80 characters that says what `s` is, for a report: white space collapsed, nothing invisible.
pub(crate) fn short(s: &str) -> String {
    crate::TEXT_WORK.fetch_add(s.len(), std::sync::atomic::Ordering::Relaxed);
    let one = s.split_whitespace().take(40).collect::<Vec<_>>().join(" ");
    clean(&one, 80)
}
