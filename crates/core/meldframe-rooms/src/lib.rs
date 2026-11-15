//! Many rooms in one process: the activity log of accepted ops.
//!
//! This crate is portable: no IO, no async runtime, no threads, no network, no wall clock. It sits on
//! `meldframe-host`, which holds the single room, the store seam and the admission gate. It is a separate
//! crate because `meldframe-host` is near its 2,000-line ceiling (D1).

mod activity;
mod logged;

pub use activity::{ActivityEntry, ActivityError, ActivityKind, ActivityLog, MemoryActivityLog};
pub use logged::{Logged, accept_logged};
