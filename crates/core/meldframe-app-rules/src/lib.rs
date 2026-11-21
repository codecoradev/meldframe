//! Rules of the app layer that hold a little state of their own and are given what they need, split from
//! `meldframe-app` to keep it under the size ceiling (2,000 lines a crate): the connection timing (`link`), the retries
//! of a refused op (`retry`), the review record, the checkpoint list and the plans of a revert and a restore
//! (`review`), the mode picks and the check of a list of variable commands (`modes`), and how much an action
//! changes (`level`), and, for a laid-out canvas, the commands of a drop (`drop`) and of a resize (`resize`): both
//! pure functions of the nodes and the laid-out boxes.
//!
//! None of it holds an `AppState`, an event list or a replica, and nothing reads a clock or does I/O: time comes in
//! as a number. The state machine that owns the state, the replica and the events stays in `meldframe-app`, and calls
//! these.

pub mod arrow;
pub mod drop;
pub mod level;
pub mod link;
pub mod modes;
pub mod resize;
pub mod retry;
pub mod review;
