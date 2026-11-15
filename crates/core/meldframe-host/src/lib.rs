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
//! together with its permit. The block
//! below is the whole path and compiles.
//!
//! ```
//! use meldframe_host::{Gate, MemoryStore, Op, Principal, Room};
//! use meldframe_protocol::Command;
//!
//! let node = serde_json::from_value(serde_json::json!({"id": "n1", "type": "frame"})).unwrap();
//! let op = Op::new("k1", Command::AddNode { node, index: None });
//! let who = Principal::new("ws_1", "user:a");
//! let adm = Gate::new().submit(&who, vec![op]).unwrap().admitted.remove(0);
//! let mut room = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
//! assert_eq!((adm.op().key.as_str(), adm.permit().key()), ("k1", "k1"));
//! assert_eq!(room.accept(adm).unwrap().seq, 1);
//! ```
//!
//! That a write without a permit, a permit built by hand, or an op paired with a permit by hand does not
//! compile is pinned by the `trybuild` cases of `meldframe-rooms` (`tests/ui`; this crate is at its size ceiling).

mod gate;
mod op;
mod refusal;
mod room;
mod store;

pub use gate::{Admission, Gate, Permit, Principal, Report};
pub use op::Op;
pub use refusal::Refusal;
pub use room::{Accepted, REPLAY_PAGE, Room, RoomError, SnapshotWarning};
pub use store::{Appended, MemoryStore, NewOp, Snapshot, Store, StoreError, StoredOp};
