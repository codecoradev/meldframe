//! The approval path for a call with policy Ask (ADR-0010, E2.4): see [`Approvals`].
//!
//! This crate is portable: no IO, no async runtime, no threads, no network, no wall clock. It sits on
//! `meldframe-host` (the gate) and `meldframe-rooms` (the command summary of the activity log). It is a
//! separate crate because `meldframe-host` and `meldframe-rooms` are at their size ceilings (D1, ADR-0011).

mod approvals;

pub use approvals::{
    Approval, ApprovalGate, ApprovalRefusal, ApprovalState, Approvals, DecideError, Decision,
    Verdict,
};
