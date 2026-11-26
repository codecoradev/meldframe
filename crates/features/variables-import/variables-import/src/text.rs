//! Text for a report: what is cut and made safe before the text of a style sheet is echoed to the reader of a report.
//!
//! Rule U9: the set of hidden characters is the one of `meldframe-text`, not a list of this crate. The report is text
//! shown to a person, so it takes `Profile::Display`: a zero-width joiner or a variation selector stays where it makes
//! an emoji and every other control, format, bidirectional or tag character goes. What becomes a name or an id of the
//! file is not made here: `names::variable_name` keeps ASCII letters and digits only, so it holds no hidden character
//! to remove (the strict profile would remove nothing from it).

use meldframe_text::{Profile, sanitize, sanitize_clip};

/// `s` made safe to show (`Profile::Display`), cut to `max` characters with `...` at the cut.
pub(crate) fn clean(s: &str, max: usize) -> String {
    let safe = sanitize(s, Profile::Display);
    if safe.chars().count() <= max {
        return safe.into_owned();
    }
    // Cut shorter by the ellipsis; a cut inside an emoji sequence leaves no joiner behind (`sanitize_clip`).
    let mut out = sanitize_clip(&safe, Profile::Display, max.saturating_sub(3), usize::MAX);
    out.push_str("...");
    out
}

/// One line of at most 80 characters that says what `s` is, for a report: white space collapsed, nothing hidden.
pub(crate) fn short(s: &str) -> String {
    crate::TEXT_WORK.fetch_add(s.len(), std::sync::atomic::Ordering::Relaxed);
    let one = s.split_whitespace().take(40).collect::<Vec<_>>().join(" ");
    clean(&one, 80)
}
