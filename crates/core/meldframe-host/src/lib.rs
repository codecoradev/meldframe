//! Rooms, jobs, store, admission gate and activity log.
//!
//! This crate is portable: no IO, no async runtime, no network. Adapters for Postgres and the
//! transports live in later changes (E1.2, E1.3, E1.5).
//!
//! The admission gate ([`Gate`]) is the one way to get a [`Permit`]; every transport submits through it
//! (ADR-0010).
//!
//! # A write needs a permit
//!
//! `Room::accept(op, permit)` is the only way to write, and only the gate makes a permit. The first
//! block is the whole path and compiles. The blocks after it must not compile; each names the error it
//! is meant to fail with, so one cannot pass by failing for another reason (a typo, a missing import).
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
//! assert_eq!(room.accept(adm.op, adm.permit).unwrap().seq, 1);
//! ```
//!
//! A write without a permit does not compile (E0061, too few arguments):
//!
//! ```compile_fail,E0061
//! use meldframe_host::{MemoryStore, Op, Room};
//! use meldframe_protocol::Command;
//!
//! let mut room = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
//! let op = Op::new("k1", Command::RemoveNode { id: "n1".into() });
//! room.accept(op);
//! ```
//!
//! A permit cannot be built outside the gate, by struct literal (E0451, private fields):
//!
//! ```compile_fail,E0451
//! use meldframe_host::Permit;
//! let _ = Permit { actor: "a".into(), workspace: "w".into(), key: "k".into() };
//! ```
//!
//! nor by constructor (E0624, private function):
//!
//! ```compile_fail,E0624
//! use meldframe_host::Permit;
//! let _ = Permit::new("a", "w", "k", meldframe_module::RiskTier::Read);
//! ```

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
