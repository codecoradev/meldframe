//! Rooms, jobs, store, admission gate and activity log.
//!
//! This crate is portable: no IO, no async runtime, no network. Adapters for Postgres and the
//! transports live in later changes (E1.2, E1.3, E1.5).
//!
//! The admission gate ([`Gate`]) is the one way to get a [`Permit`]; every transport submits through it
//! (ADR-0010).

mod gate;
mod op;
mod refusal;

pub use gate::{Admission, Gate, Permit, Principal, Report};
pub use op::Op;
pub use refusal::Refusal;
