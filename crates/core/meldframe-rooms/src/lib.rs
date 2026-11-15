//! Many rooms in one process: the activity log of accepted ops and the room registry.
//!
//! This crate is portable: no IO, no async runtime, no threads, no network, no wall clock. It sits on
//! `meldframe-host`, which holds the single room, the store seam and the admission gate. It is a separate
//! crate because `meldframe-host` is near its 2,000-line ceiling (D1).

mod activity;
mod logged;
mod registry;

pub use activity::{ActivityEntry, ActivityError, ActivityKind, ActivityLog, MemoryActivityLog};
pub use logged::{Logged, accept_logged};
pub use registry::{Opened, RegistryError, Rooms};
