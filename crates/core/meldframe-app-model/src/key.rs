//! A key press as an input of the app layer (audit finding T7a, issue 324, decision 43 clause Q1). A front end reports the key
//! the way the platform gives it (the character or the name of the key, the physical code and the modifiers) and says where
//! the focus is; the app layer decides what the key means (`meldframe-app-rules`, `keys`), so a second front end does not
//! write the keyboard again. The text of a key is never shown and never stored: it is only matched against a table.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The modifier keys held with a key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Modifiers {
    #[serde(default)]
    pub ctrl: bool,
    /// Command on a Mac, the Windows key elsewhere.
    #[serde(default)]
    pub meta: bool,
    /// Option on a Mac.
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
}

impl Modifiers {
    /// Ctrl or Command: the key of shortcuts. Either one counts on every platform, as the editor has always taken it.
    pub fn command(&self) -> bool {
        self.ctrl || self.meta
    }
}

/// Who owns the keys at the place the focus is. The page knows this from its DOM; the app layer cannot, so the front end
/// says it, and the app layer applies the rule: what the focus owns is not an editor key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum KeyFocus {
    /// Nothing owns the keys: the canvas, or no element in particular.
    #[default]
    Page,
    /// A button, a link or another control that Enter activates: Enter is the control's, the other keys are the editor's.
    Control,
    /// A card of the layout, a menu or a dialog: it keeps the plain keys for what is inside it (Delete and the tool letters must
    /// not reach the canvas). Combinations with Ctrl, Command or Option, such as undo and Option+1, still go to the editor.
    Region,
    /// A field that takes text: it keeps every key.
    TextField,
}

/// A key pressed (`Action::Key`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyInput {
    /// The character or the name of the key as the platform gives it (`KeyboardEvent.key`: `v`, `?`, `Delete`, `ArrowUp`,
    /// `Escape`). A letter matches in either case.
    pub key: String,
    /// The physical key (`KeyboardEvent.code`: `Digit1`). Option changes the character on a Mac, so the digit keys that toggle the
    /// panels are matched by it. It may be empty when the front end has none.
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub modifiers: Modifiers,
    /// The key is repeating because it is held: a held key does not undo a step for each repeat, so it is no editor key.
    #[serde(default)]
    pub repeat: bool,
    /// An input method is composing text: the key is the input method's, never an action.
    #[serde(default)]
    pub composing: bool,
    /// Where the focus is.
    #[serde(default)]
    pub focus: KeyFocus,
}
