//! A key press as an input of the app layer (audit finding T7a, issue 324, decision 43 clause Q1). A front end reports the key
//! the way the platform gives it (the character or the name of the key, the physical code and the modifiers) and says where
//! the focus is; the app layer decides what the key means (`meldframe-app-rules`, `keys`), so a second front end does not
//! write the keyboard again. The text of a key is never shown and never stored: it is only matched against a table.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use meldframe_camera::Modifiers;

/// Who owns the keys at the place the focus is. The page knows this from its DOM; the app layer cannot, so the front end
/// says it, and the app layer applies the rule: what the focus owns is not an editor key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum KeyFocus {
    /// Nothing owns the keys: the canvas, or no element in particular.
    Page,
    /// A button, a link or another control that Enter activates: Enter is the control's, the other keys are the editor's.
    Control,
    /// A card of the layout, a menu or a dialog: it keeps the plain keys for what is inside it (Delete and the tool letters must
    /// not reach the canvas). Combinations with Ctrl, Command or Option, such as undo and Option+1, still go to the editor.
    Region,
    /// A field that takes text: it keeps every key.
    TextField,
}

/// A key pressed (`Action::Key`). A key the app layer does not answer, a key of an input method that is composing, a repeat, and
/// a key the focus owns (a field, a menu, a dialog) make no action and no event; one it answers raises `Event::KeyHandled`. The
/// keys it answers are the tool letters, Delete, undo and redo, the arrow keys, Enter and Escape, the panel keys, `?` and Ctrl or
/// Command+K (`meldframe-app-rules`, `keys`).
///
/// What the host still does, because the app layer has no state for it:
///
/// - It does not send an arrow key while a drag is going on (the TypeScript returned before dispatching one). The app layer has
///   no gesture state. An arrow key with no selection may be sent: it is a no-op there.
/// - It does not send Ctrl or Command+K while a menu is open (a menu keeps its open state in the component library, and
///   `KeyFocus` cannot say that one is open elsewhere), and it still calls `preventDefault` on that key itself, as the
///   TypeScript did.
/// - It keeps the keys of the camera and of a gesture: zoom, fit, Space, and the Escape that abandons a drag. Escape here is
///   the app's own, for a text edit or an overlay, and only without a modifier.
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
    /// Where the focus is. Required: a host that leaves it out would let a key typed in a field act on the selection, so there
    /// is no default, and a key with no `focus` is refused when the action is read.
    pub focus: KeyFocus,
}
