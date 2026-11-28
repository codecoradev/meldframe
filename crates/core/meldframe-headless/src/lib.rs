//! A made-up second front end for the app layer: a headless command line shell. It reads a script, sends the
//! actions the script stands for to a `meldframe_app::App`, answers the effects as a platform would, and writes
//! down what it saw. It has no window, no toolkit and no pixels, and it imports no `ui`, `feature-ui`, web or
//! Rune crate: it is the living proof of the thin UI rule U8 (a second front end drives the same state machine
//! with no logic moved) and, with `cargo xtask toolkit-swap`, of the toolkit swap rule U10.
//!
//! # Script
//!
//! One step a line; a blank line and a line that starts with `#` are skipped. A step that drives the app is read
//! into an `Action` through the action's own JSON shape, so the harness writes no rule.
//!
//! ```text
//! open <file>                     load the nodes (a JSON array of nodes) and connect
//! select <id>... | clear          Action::Select, Action::ClearSelection
//! tool <name>                     Action::SetTool
//! move <id> <x> <y>               Action::Move;  moveby <dx> <dy> moves the selection
//! drop <id> <x> <y> <dx> <dy>     Action::DropNodes with one node
//! resize <id> <w> <h>             Action::Resize
//! handle <id> <handle> <dx> <dy>  Action::Gesture, a handle of the selection box (nw n ne e se s sw w)
//! draw <kind> <x1> <y1> <x2> <y2> Action::Gesture, a node drawn with a tool
//! camera viewport <w> <h> | pan <dx> <dy> | zoom <command>
//! pointer <phase> <x> <y> [flags]  Action::Pointer: phase down, move, up, cancel or leave; (x, y) a screen point; flags
//!                                 shift ctrl alt meta space touch middle secondary
//! key <combo>                     Action::Key as the page: r, Delete, ctrl+z, shift+?
//! undo | redo                     Action::Undo, Action::Redo
//! palette <query> [run <id> [<cx> <cy>]]   list the entries, or run one
//! export <id> png [<scale>] | html         plan the export; the PNG is drawn by a fake shell
//! action <json>                   any Action in its JSON shape
//! bytes <n>                       from here on the fake shell reports each image it draws as <n> bytes (a size over a limit
//!                                 reaches the app as data)
//! state | nodes                   print the state summary, the nodes
//! strict                          from here on a refusal fails the run
//! expect refused <code> | clean | node <id> <field> <value> | state <pointer> <json> | delivered <w> <h>
//! expect preview <pointer> <json>          what the gesture going on would do (`App::pointer_preview`; null when none)
//! expect order <parent> <id>...            the children of a node in sibling order
//! expect effect <kind> [<field> <json>]    the last step returned an effect of that kind, with that value in the field
//! ```
//!
//! The output is plain text: each step, then the kinds of the effects and events it made and its refusals. The
//! run stops at the first step that fails, and the command line exits non-zero.
//!
//! A drag is a series of `pointer` steps (the machine of `meldframe-app-pointer` decides what each does), or one `drop`, `handle`
//! or `draw` step, which are the same intents sent whole. In a strict run a refusal is held until the next step: an
//! `expect refused <code>` excuses it, any other step fails the run at the line that was refused.

mod run;
mod script;
mod shell;

pub use run::{Files, Report, run};
pub use script::{Expect, Form, Step, key, parse};
pub use shell::{ACTOR, Seen, Shell};
