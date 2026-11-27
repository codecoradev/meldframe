//! What a key press makes: a pure function of the key and where the focus is. The state of the app (the selection, the open
//! edit, the layout on the canvas) decides afterwards whether the thing it makes changes anything; this only says what the key
//! is.

use super::table::{Combo, Makes, TABLE};
use meldframe_app_model::{KeyFocus, KeyInput};

impl Combo {
    /// Whether the key press is this combo. A key of one character is a letter or a sign and matches in either case; a named
    /// key (`Delete`) matches exactly. With a code, the physical key decides.
    pub fn matches(&self, input: &KeyInput) -> bool {
        let m = &input.modifiers;
        if m.command() != self.command || !self.alt.allows(m.alt) || !self.shift.allows(m.shift) {
            return false;
        }
        match self.code {
            Some(code) if self.typed.is_none() || !input.code.is_empty() => input.code == code,
            Some(_) => input.key == self.key || self.typed == Some(input.key.as_str()),
            None if self.key.chars().count() == 1 => input.key.eq_ignore_ascii_case(self.key),
            None => input.key == self.key,
        }
    }
}

/// Whether the focus keeps the key for itself, so that it is not an editor key: a field keeps every key; a card of the layout,
/// a menu or a dialog keeps the plain ones (not a combination with Ctrl, Command or Option); a control keeps Enter.
fn owned(input: &KeyInput) -> bool {
    let m = &input.modifiers;
    match input.focus {
        KeyFocus::Page => false,
        KeyFocus::Control => input.key == "Enter" && !m.command() && !m.shift && !m.alt,
        KeyFocus::Region => !m.command() && !m.alt,
        KeyFocus::TextField => true,
    }
}

/// What the key press makes, or `None` when it is not an editor key the app layer answers: a repeat of a held key, a key of an
/// input method that is composing, a key the focus owns, a key the front end answers (`Makes::Host`) and a key that is in no row.
pub fn decide(input: &KeyInput) -> Option<Makes> {
    if input.composing || owned(input) {
        return None;
    }
    TABLE
        .iter()
        .flat_map(|g| g.rows)
        .flat_map(|r| r.bindings)
        .find(|b| b.makes != Makes::Host && b.combo.matches(input))
        .filter(|b| b.repeats || !input.repeat)
        .map(|b| b.makes)
}
