//! The controls the Rune views share: a button and a line of status, drawn from the tokens (U4). The crate holds no state
//! of the product and no rule (U2): a control is given its label, whether it can be used and what a press does.

mod tokens;

use rune::{El, el, text};
pub use tokens::*;

/// A button of the bar. `enabled` false draws it faded and turns a press into nothing. The key is what a test and the
/// accessibility tree find it by, and the label is what a person reads and what a screen reader says.
pub fn button<S: 'static>(
    key: &'static str,
    label: &'static str,
    enabled: bool,
    on_press: impl Fn(&mut S) + 'static,
) -> El {
    let t = colours();
    let face = text(label).font(type_::LABEL).color(t.text);
    let b = el()
        .key(key)
        .role("button")
        .label(label)
        .size(BUTTON_WIDTH, BUTTON_HEIGHT)
        .radius(radius::SM)
        .fill(t.control_track)
        .center()
        .disabled(!enabled)
        .child(face);
    if enabled {
        b.focusable(true).on_press(on_press)
    } else {
        b.opacity(DISABLED_OPACITY)
    }
}

/// A line of status text. The text is given, already safe to show (U9).
pub fn status_line(key: &'static str, content: impl Into<String>) -> El {
    text(content)
        .key(key)
        .font(type_::LABEL)
        .color(colours().text_secondary)
        .w(STATUS_WIDTH)
}
