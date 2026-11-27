//! The command palette as a function of the app state and a query: the entries it lists, how a typed query narrows and ranks them, the keys that move its highlight, and what running an entry makes.
//!
//! The TypeScript palette (`packages/web-shell/src/palette.ts`) is not rewired: it is now a legacy copy of these rules, pinned to them
//! by the same cases in `meldframe-app-unit-tests` (`palette.rs`) and retired with the web clients (E8.6.1).

mod entries;
mod plan;
mod rank;

pub use entries::{Command, Entry, Zoom, entries};
pub use plan::{Plan, plan};
pub use rank::{KeyResult, filter, first_enabled, highlight_at, on_key, results_text};
