//! The key table: one row for each thing a person can do with a key or a gesture, in the groups of the shortcut sheet. A row has
//! its words (the sheet shows them), its combos, and what each combo makes. A combo that the app layer answers makes an
//! [`Makes`] that `meldframe-app` carries out; the keys and gestures that the front end answers (the camera, a drag, the field of
//! a text edit) are in the table too, as [`Makes::Host`], so the sheet lists everything from one place and says who answers what.
//!
//! The rows are the ones the TypeScript editor answered to (`packages/web-shell/src/shortcuts.ts`, `web-document/src/edit.ts`,
//! `web-shell/src/keys.ts`), with the modifiers each handler asked for.

use meldframe_app_model::{Direction, Overlay, Panel, Tool, ZoomCommand};

/// Whether a modifier has to be held, has to be free, or does not matter for a combo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    Not,
    Yes,
    Either,
}

impl Held {
    /// Whether a key pressed with the modifier `down` (or not) fits.
    pub const fn allows(self, down: bool) -> bool {
        match self {
            Held::Not => !down,
            Held::Yes => down,
            Held::Either => true,
        }
    }
}

/// A key with the modifiers it is pressed with. `command` is Ctrl or Command (the sheet writes the one of the platform); a key
/// is a letter (matched in either case), a named key (`Delete`) or, for a gesture, the word the sheet shows (`Scroll`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Combo {
    pub key: &'static str,
    /// When set, the physical key decides (`Digit1`), and `key` is what the sheet shows.
    pub code: Option<&'static str>,
    /// What the key types with Shift held on a US layout (`!` for `1`, `_` for `-`). Set, the combo also answers this character:
    /// with a `code`, when the front end gives no physical code (`KeyInput::code` may be empty; a press with a code is still
    /// decided by the code), and without a `code`, always, as the character `key` does.
    pub typed: Option<&'static str>,
    pub command: bool,
    pub alt: Held,
    pub shift: Held,
}

impl Combo {
    /// The key pressed alone: Ctrl, Command, Option and Shift all free.
    pub const fn key(key: &'static str) -> Combo {
        Combo {
            key,
            code: None,
            typed: None,
            command: false,
            alt: Held::Not,
            shift: Held::Not,
        }
    }
    pub const fn command(self) -> Combo {
        Combo {
            command: true,
            ..self
        }
    }
    pub const fn alt(self, alt: Held) -> Combo {
        Combo { alt, ..self }
    }
    pub const fn shift(self, shift: Held) -> Combo {
        Combo { shift, ..self }
    }
    pub const fn typed(self, typed: &'static str) -> Combo {
        Combo {
            typed: Some(typed),
            ..self
        }
    }
    pub const fn code(self, code: &'static str) -> Combo {
        Combo {
            code: Some(code),
            ..self
        }
    }
}

/// What a key makes in the app layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Makes {
    SetTool(Tool),
    Undo,
    Redo,
    /// Delete the selection; nothing when none is selected.
    DeleteSelection,
    /// An arrow key with the selection: the app decides whether anything moves (the layout on the canvas, a child of a flow).
    Nudge(Direction),
    /// Open the edit of the one selected text node; nothing for any other selection.
    BeginTextEdit,
    /// Escape: cancel the text being edited, else close the overlay that is open.
    Cancel,
    TogglePanel(Panel),
    ToggleOverlay(Overlay),
    /// A zoom key: the camera runs the command (`Action::Camera`). The wheel, the pinch and a drag with Space held are gestures.
    Zoom(ZoomCommand),
    /// The front end answers it (a gesture, the field of a text edit): the app layer makes nothing of it.
    Host,
}

/// A combo and what it makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub combo: Combo,
    pub makes: Makes,
    /// The key goes on making it while it is held (a repeat is no editor key for any other binding: a held Delete or undo key
    /// does not delete or undo a step for each repeat). The zoom keys `+`, `=` and `-` do, as they always did.
    pub repeats: bool,
}

/// One row of the sheet: a thing to do, and the ways to do it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub id: &'static str,
    pub name: &'static str,
    /// The editor answers it only while "Auto layout on canvas" is on: the sheet leaves the row out while it is off.
    pub needs_auto_layout: bool,
    pub bindings: &'static [Binding],
}

/// A group of rows with a title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Group {
    pub id: &'static str,
    pub title: &'static str,
    pub rows: &'static [Row],
}

const fn bind(combo: Combo, makes: Makes) -> Binding {
    Binding {
        combo,
        makes,
        repeats: false,
    }
}
const fn row(id: &'static str, name: &'static str, bindings: &'static [Binding]) -> Row {
    Row {
        id,
        name,
        needs_auto_layout: false,
        bindings,
    }
}
/// A tool letter: Shift is free (Caps Lock and a Shift held by habit still pick the tool).
const fn tool(letter: &'static str, tool: Tool) -> Binding {
    bind(Combo::key(letter).shift(Held::Either), Makes::SetTool(tool))
}
const fn zoom_key(combo: Combo, command: ZoomCommand) -> Binding {
    bind(combo, Makes::Zoom(command))
}
/// A zoom step: a held key goes on zooming.
const fn zoom_step(combo: Combo, command: ZoomCommand) -> Binding {
    Binding {
        repeats: true,
        ..zoom_key(combo, command)
    }
}
const fn on_row(combo: Combo) -> Binding {
    bind(combo, Makes::Host)
}

/// The table, in the order of the sheet.
pub static TABLE: &[Group] = &[
    Group {
        id: "tools",
        title: "Tools",
        rows: &[
            row("tool-select", "Select", &[tool("V", Tool::Select)]),
            row("tool-frame", "Frame", &[tool("F", Tool::Frame)]),
            row("tool-rectangle", "Rectangle", &[tool("R", Tool::Rectangle)]),
            row("tool-ellipse", "Ellipse", &[tool("E", Tool::Ellipse)]),
            row("tool-text", "Text", &[tool("T", Tool::Text)]),
        ],
    },
    Group {
        id: "edit",
        title: "Edit",
        rows: &[
            row(
                "undo",
                "Undo",
                &[bind(Combo::key("Z").command(), Makes::Undo)],
            ),
            row(
                "redo",
                "Redo",
                &[
                    bind(Combo::key("Z").command().shift(Held::Yes), Makes::Redo),
                    bind(Combo::key("Y").command(), Makes::Redo),
                ],
            ),
            row(
                "delete",
                "Delete the selection",
                &[
                    bind(delete_key("Delete"), Makes::DeleteSelection),
                    bind(delete_key("Backspace"), Makes::DeleteSelection),
                ],
            ),
            Row {
                needs_auto_layout: true,
                ..row(
                    "move-in-flow",
                    "Move one place in an auto layout frame",
                    &[
                        bind(Combo::key("ArrowUp"), Makes::Nudge(Direction::Up)),
                        bind(Combo::key("ArrowDown"), Makes::Nudge(Direction::Down)),
                        bind(Combo::key("ArrowLeft"), Makes::Nudge(Direction::Left)),
                        bind(Combo::key("ArrowRight"), Makes::Nudge(Direction::Right)),
                    ],
                )
            },
            row(
                "edit-text",
                "Edit the selected text",
                &[bind(Combo::key("Enter"), Makes::BeginTextEdit)],
            ),
            row(
                "commit-text",
                "Finish editing the text",
                &[on_row(Combo::key("Enter").command())],
            ),
            // The words are the sheet's: the app layer answers Escape for a text edit and an overlay (with no modifier held),
            // and the canvas keeps the Escape that abandons a drag, so "a drag" is the front end's half of the row.
            row(
                "cancel",
                "Cancel a drag or a text edit",
                &[bind(Combo::key("Escape"), Makes::Cancel)],
            ),
        ],
    },
    Group {
        id: "view",
        title: "View",
        rows: &[
            row(
                "zoom-in",
                "Zoom in",
                &[
                    zoom_step(Combo::key("+").shift(Held::Either), ZoomCommand::In),
                    zoom_step(Combo::key("="), ZoomCommand::In),
                ],
            ),
            row(
                "zoom-out",
                "Zoom out",
                // Read by the character, as the zoom in rows are: the numpad minus and a layout that puts `-` on another
                // physical key (German) zoom out too, and Shift and the minus key types `_` on a US layout.
                &[zoom_step(
                    Combo::key("-").typed("_").shift(Held::Either),
                    ZoomCommand::Out,
                )],
            ),
            // Shift and a digit, by the physical key: the character it types depends on the layout (`!` on a US one).
            row(
                "fit-all",
                "Fit all",
                &[zoom_key(
                    Combo::key("1").code("Digit1").typed("!").shift(Held::Yes),
                    ZoomCommand::Fit,
                )],
            ),
            row(
                "fit-selection",
                "Zoom to selection",
                &[zoom_key(
                    Combo::key("2").code("Digit2").typed("@").shift(Held::Yes),
                    ZoomCommand::FitSelection,
                )],
            ),
            row("scroll", "Pan", &[on_row(Combo::key("Scroll"))]),
            row(
                "pan-space",
                "Pan with the pointer",
                &[on_row(Combo::key("Space + drag"))],
            ),
            row(
                "zoom-wheel",
                "Zoom at the pointer",
                &[on_row(Combo::key("Scroll").command())],
            ),
        ],
    },
    Group {
        id: "selection",
        title: "Selection",
        rows: &[
            row(
                "add-select",
                "Add to or take from the selection",
                &[on_row(Combo::key("Click").shift(Held::Yes))],
            ),
            row(
                "edit-dblclick",
                "Edit a text",
                &[on_row(Combo::key("Double-click"))],
            ),
        ],
    },
    Group {
        id: "panels",
        title: "Panels and help",
        rows: &[
            row(
                "layers-panel",
                "Layers panel",
                &[bind(
                    digit("1", "Digit1"),
                    Makes::TogglePanel(Panel::Layers),
                )],
            ),
            row(
                "right-panel",
                "Right panel",
                &[bind(
                    digit("2", "Digit2"),
                    Makes::TogglePanel(Panel::Properties),
                )],
            ),
            row(
                "shortcuts",
                "Shortcuts",
                &[bind(
                    Combo::key("?").shift(Held::Either),
                    Makes::ToggleOverlay(Overlay::Help),
                )],
            ),
            row(
                "palette",
                "Command palette",
                &[bind(
                    Combo::key("K").command(),
                    Makes::ToggleOverlay(Overlay::Palette),
                )],
            ),
        ],
    },
];

/// Delete and Backspace: Option and Shift are free, as they were in the TypeScript editor.
const fn delete_key(key: &'static str) -> Combo {
    Combo::key(key).alt(Held::Either).shift(Held::Either)
}

/// Option and a digit, by the physical key: Option changes the character on a Mac. Shift is free.
const fn digit(key: &'static str, code: &'static str) -> Combo {
    Combo::key(key)
        .code(code)
        .alt(Held::Yes)
        .shift(Held::Either)
}
