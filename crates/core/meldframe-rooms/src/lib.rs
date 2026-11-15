//! Many rooms in one process: the activity log of accepted ops and the room registry (open, close, a bound,
//! and closing idle rooms by a caller-supplied clock: `Rooms::close_idle`).
//!
//! This crate is portable: no IO, no async runtime, no threads, no network, no wall clock. It sits on
//! `meldframe-host`, which holds the single room, the store seam and the admission gate. It is a separate
//! crate because `meldframe-host` is near its 2,000-line ceiling (D1).
//!
//! # An admission cannot be forged
//!
//! An op cannot be paired with a permit by hand: `Admission` has no public fields, and `Permit` has no public
//! constructor, fields or `Clone`. `tests/compile_fail.rs` proves it with `trybuild`: each case in `tests/ui` fails
//! with the error code named in its first line, with the exact compiler output in its `.stderr`, and
//! `pass_legal_path.rs` is the companion that compiles. After a toolchain bump, regenerate the output with
//! `TRYBUILD=overwrite cargo test -p meldframe-rooms --test compile_fail`.

mod activity;
mod logged;
mod registry;

pub use activity::{ActivityEntry, ActivityError, ActivityKind, ActivityLog, MemoryActivityLog};
pub use logged::{Logged, accept_logged};
pub use registry::{Opened, RegistryError, Rooms};
