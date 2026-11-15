//! Room, store seam and admission gate; jobs come later.
//!
//! This crate is portable: no IO, no async runtime, no network. Adapters for Postgres and the
//! transports live in later changes (E1.2, E1.3, E1.5).
//!
//! The admission gate ([`Gate`]) is the one way to get a [`Permit`]; every transport submits through it
//! (ADR-0010).
//!
//! # A write needs a permit
//!
//! `Room::accept(admission)` is the only way to write, and only the gate makes an [`Admission`], an op sealed
//! together with its permit: `Gate::submit(&who, ops)` returns them, `Room::accept` takes one (`tests/room.rs`
//! runs the whole path).
//!
//! That a write without a permit, a permit built by hand, or an op paired with a permit by hand does not
//! compile is pinned by the `trybuild` cases of `meldframe-rooms` (`tests/ui`; this crate is at its size ceiling).

mod gate;
mod op;
mod refusal;
mod room;
mod store;

pub use gate::{Admission, Gate, NodeFacts, Permit, Principal, Report};
pub use op::Op;
pub use refusal::Refusal;
pub use room::{Accepted, REPLAY_PAGE, Room, RoomError, SnapshotWarning};
pub use store::{Appended, MemoryStore, NewOp, Snapshot, Store, StoreError, StoredOp};
