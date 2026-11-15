//! Many rooms in one process: the activity log of accepted ops and the room registry.
//!
//! This crate is portable: no IO, no async runtime, no threads, no network, no wall clock. It sits on
//! `meldframe-host`, which holds the single room, the store seam and the admission gate. It is a separate
//! crate because `meldframe-host` is near its 2,000-line ceiling (D1).
//!
//! # An admission cannot be forged
//!
//! An op cannot be paired with a permit by hand: `Admission` has no public fields, so no struct literal
//! builds one (E0451),
//!
//! ```compile_fail,E0451
//! # use meldframe_host::{Admission, Op};
//! # use meldframe_protocol::Command;
//! let op = Op::new("k1", Command::RemoveNode { id: "n1".into() });
//! let _ = Admission { op, permit: unimplemented!() };
//! ```
//!
//! neither part can be replaced in an admission the gate made (E0616, private field),
//!
//! ```compile_fail,E0616
//! # use meldframe_host::{Gate, Op, Principal};
//! # use meldframe_protocol::Command;
//! # let who = Principal::new("ws_1", "a");
//! # let ops = vec![Op::new("k1", Command::RemoveNode { id: "n1".into() }), Op::new("k2", Command::RemoveNode { id: "n1".into() })];
//! # let mut v = Gate::with_policy(|_, _| meldframe_module::Policy::Auto).submit(&who, ops).unwrap().admitted;
//! # let (mut a, mut b) = (v.remove(0), v.remove(0));
//! std::mem::swap(&mut a.op, &mut b.op);
//! std::mem::swap(&mut a.permit, &mut b.permit);
//! ```
//!
//! and the parts of two admissions cannot be swapped into the room (E0616 for the private fields):
//!
//! ```compile_fail,E0616
//! # use meldframe_host::{Gate, MemoryStore, Op, Principal, Room};
//! # use meldframe_protocol::Command;
//! # let who = Principal::new("ws_1", "a");
//! # let ops = vec![Op::new("k1", Command::RemoveNode { id: "n1".into() }), Op::new("k2", Command::RemoveNode { id: "n1".into() })];
//! # let mut v = Gate::with_policy(|_, _| meldframe_module::Policy::Auto).submit(&who, ops).unwrap().admitted;
//! # let (a, b) = (v.remove(0), v.remove(0));
//! # let mut room = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
//! let _ = room.accept(a.op, b.permit);
//! ```
//!
//! The first doctest of `meldframe-host` is the companion that compiles: the gate's admission goes to
//! `Room::accept` whole.
//!
//! ```
//! # use meldframe_host::{Gate, Op, Principal};
//! # use meldframe_protocol::Command;
//! # let who = Principal::new("ws_1", "a");
//! # let op = Op::new("k1", Command::RemoveNode { id: "n1".into() });
//! # let adm = Gate::with_policy(|_, _| meldframe_module::Policy::Auto).submit(&who, vec![op]).unwrap().admitted.remove(0);
//! assert_eq!((adm.op().key.as_str(), adm.permit().key()), ("k1", "k1"));
//! ```

mod activity;
mod logged;
mod registry;

pub use activity::{ActivityEntry, ActivityError, ActivityKind, ActivityLog, MemoryActivityLog};
pub use logged::{Logged, accept_logged};
pub use registry::{Opened, RegistryError, Rooms};
